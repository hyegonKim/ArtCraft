//! The plug-in host: loading, instantiating and calling WebAssembly (`.wasm`) and native
//! Photoshop `.8bf` / Mach-O (`.plugin`) filter plug-ins under resource limits.

use std::sync::{Arc, Mutex};

use wasmi::{
    Config, EnforcedLimits, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc, TypedResumableCall, WasmParams,
    WasmResults,
};

use crate::manifest::{Area, Kind, Manifest, printable_name, trim_name};
use crate::native_vm::{Filter8bfHost, PluginDescriptor, PluginFormat, PluginHostError};
use crate::{Error, Result};

/// The plug-in ABI version this host implements (`pc_abi_version` must return it).
pub const ABI_VERSION: i32 = 1;

/// Fuel handed out per slice; between slices the host checks the deadline and the abort flag.
const FUEL_SLICE: u64 = 20_000_000;

/// Resource limits for running plug-ins.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// Largest module accepted.
    pub max_module_bytes: usize,
    /// Cap on a plug-in's linear memory.
    pub max_memory_bytes: usize,
    /// Cap on the linear memory of all of one run's instances together: bands run in parallel
    /// only as far as their [`Limits::max_memory_bytes`] fit in it, so a run's peak memory does
    /// not grow with the number of cores (#8603).
    pub max_total_memory_bytes: usize,
    /// Target size of one band of `f32` pixels handed to the plug-in.
    pub band_bytes: usize,
    /// Instructions (fuel) any call may use, plus [`Limits::fuel_per_sample`] per sample for
    /// `pc_filter`.
    pub fuel_base: u64,
    pub fuel_per_sample: u64,
    /// Deepest call nesting.
    pub max_recursion_depth: usize,
    /// Wall-clock budget for one filter run (all bands), in milliseconds. Native builds only:
    /// the web build has no monotonic clock in `std`, so the fuel budget alone bounds it there.
    pub wall_time_ms: u64,
}

impl Default for Limits {
    /// The limits plug-ins run under unless a caller narrows them; see `docs/plugins.md`.
    fn default() -> Self {
        Self {
            max_module_bytes: 32 << 20,
            max_memory_bytes: 512 << 20,
            max_total_memory_bytes: 2 << 30,
            band_bytes: 4 << 20,
            fuel_base: 50_000_000,
            fuel_per_sample: 4_000,
            max_recursion_depth: 1024,
            wall_time_ms: 60_000,
        }
    }
}

impl Limits {
    /// How many bands of one run may hold an instance at once on `threads` workers: no more
    /// than [`Limits::max_memory_bytes`] fits into [`Limits::max_total_memory_bytes`], and always
    /// at least one, so a tight budget runs bands one at a time instead of refusing the run.
    pub fn parallel_bands(&self, threads: usize) -> usize {
        let fit = self.max_total_memory_bytes.checked_div(self.max_memory_bytes).unwrap_or(threads);
        threads.min(fit).max(1)
    }
}

enum Backend {
    Wasm { engine: Engine, module: Module },
    Native8bf(Box<Filter8bfHost>),
}

/// A loaded, validated plug-in. Cheap to share (`Arc`) and safe to run from several threads:
/// every run gets its own store / VM instance.
pub struct Plugin {
    manifest: Manifest,
    backend: Backend,
    limits: Limits,
    size: usize,
    source: Option<String>,
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Plugin>();
};

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin")
            .field("manifest", &self.manifest)
            .field("native_8bf", &self.is_native_8bf())
            .field("size", &self.size)
            .field("source", &self.source)
            .finish()
    }
}

/// When a run must stop: a wall-clock deadline (native) and a flag shared by parallel bands.
#[derive(Clone)]
pub(crate) struct Deadline {
    #[cfg(not(target_arch = "wasm32"))]
    at: std::time::Instant,
    abort: Arc<Mutex<Option<Error>>>,
}

impl Deadline {
    pub(crate) fn new(limits: &Limits) -> Self {
        #[cfg(target_arch = "wasm32")]
        let _ = limits;
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            at: std::time::Instant::now() + std::time::Duration::from_millis(limits.wall_time_ms),
            abort: Arc::new(Mutex::new(None)),
        }
    }
    pub(crate) fn abort(&self, error: Error) {
        // Publish the cause with the cancellation state so siblings cannot replace it with a
        // generic cancellation error, even when results are collected in a different order.
        self.abort.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_or_insert(error);
    }
    pub(crate) fn check(&self) -> Result<()> {
        if let Some(error) = self.abort.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
            return Err(error);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if std::time::Instant::now() >= self.at {
            let error = Error::Limit("time budget".into());
            self.abort(error.clone());
            return Err(error);
        }
        Ok(())
    }
}

/// One live instance: a store with its own memory limit, and the module's exports.
pub(crate) struct Live {
    store: Store<StoreLimits>,
    instance: Instance,
    memory: Memory,
}

fn wasm_err(e: wasmi::Error) -> Error {
    if e.as_trap_code() == Some(wasmi::TrapCode::OutOfFuel) { Error::Limit("instruction budget".into()) } else { Error::Trap(e.to_string()) }
}

pub(crate) fn native_err(e: PluginHostError) -> Error {
    match e {
        PluginHostError::InvalidBinary(m) | PluginHostError::UnsupportedArch(m) => Error::Module(m),
        PluginHostError::MissingEntrypoint(m) | PluginHostError::Abi(m) => Error::Abi(m),
        PluginHostError::MemoryFault(m) | PluginHostError::CpuTrap(m) => Error::Trap(m),
        PluginHostError::Limit(m) => {
            if m.contains("instruction budget") {
                Error::Limit("instruction budget".into())
            } else if m.contains("binary size") {
                Error::Module(m)
            } else {
                Error::Limit(m)
            }
        }
        PluginHostError::PluginError(code) => Error::Failed(format!(".8bf filter returned error code {code}")),
    }
}

/// Reads an unsigned LEB128 `u32` at the start of `bytes`: the value and its length.
fn leb_u32(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u32;
    for (i, &b) in bytes.iter().take(5).enumerate() {
        value |= u32::from(b & 0x7f).checked_shl(7 * i as u32)?;
        if b & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

/// Refuses a code section that declares no functions but more bytes than that count. The spec
/// makes the count the whole section then; wasmi 2.0's strict limits divide the section size by
/// the function count before validating it, so such a module (crafted or truncated) panicked.
/// Every code section is checked, since a duplicate one also reaches that division before
/// validation refuses it. Anything else, malformed or not, is left to wasmi.
fn check_code_section(bytes: &[u8]) -> Result<()> {
    if bytes.get(4..8) != Some(&[1, 0, 0, 0][..]) {
        return Ok(());
    }
    let mut pos = 8usize;
    while let Some(&id) = bytes.get(pos) {
        let Some((size, len)) = bytes.get(pos + 1..).and_then(leb_u32) else { return Ok(()) };
        let start = pos + 1 + len;
        if id == 10
            && let Some((0, count_len)) = bytes.get(start..).and_then(leb_u32)
            && size as usize > count_len
        {
            return Err(Error::Module(format!("the code section declares no functions but {size} bytes")));
        }
        pos = start.saturating_add(size as usize);
    }
    Ok(())
}

/// What a file that is not WebAssembly actually is, judged from its magic number, so the install
/// error can name the format the person picked instead of only the one we wanted (public format
/// facts: the PE/COFF, ELF, Mach-O and zip signatures). `None` for anything else. Reads with
/// `starts_with`/`get`, so a file shorter than the signature is fine.
fn format_hint(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"MZ") {
        return Some("a Windows native executable or library (a .dll, or a Photoshop plug-in such as .8bf/.8bt)");
    }
    if bytes.get(..4) == Some(&[0x7f, b'E', b'L', b'F']) {
        return Some("a Linux native library (ELF)");
    }
    if matches!(bytes.get(..4), Some(&[0xce, 0xfa, 0xed, 0xfe] | &[0xcf, 0xfa, 0xed, 0xfe] | &[0xca, 0xfe, 0xba, 0xbe] | &[0xbe, 0xba, 0xfe, 0xca])) {
        return Some("a macOS native binary (Mach-O, thin or fat)");
    }
    if matches!(bytes.get(..4), Some(&[0x50, 0x4b, 0x03, 0x04] | &[0x50, 0x4b, 0x05, 0x06])) {
        return Some("a zip archive");
    }
    None
}

fn clean_file_stem(file_hint: Option<&str>) -> Option<&str> {
    let raw = file_hint?.trim();
    if raw.is_empty() {
        return None;
    }
    let fname = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let stem = fname.rsplit_once('.').map_or(fname, |(s, _)| s).trim();
    (!stem.is_empty()).then_some(stem)
}

fn normalize_id_slug(s: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if matches!(ch, '.' | '-') && !prev_sep {
            out.push(ch);
            prev_sep = true;
        } else if !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    while out.ends_with(['_', '.', '-']) {
        out.pop();
    }
    out
}

fn synthesize_8bf_manifest(desc: &PluginDescriptor, file_hint: Option<&str>) -> Result<Manifest> {
    let stem = clean_file_stem(file_hint);
    let ep_fallback = (!matches!(desc.entrypoint_symbol.as_str(), "" | "PluginMain" | "_PluginMain")).then_some(desc.entrypoint_symbol.as_str());
    let slug = [&desc.pipl.match_name[..], &desc.pipl.name[..]]
        .into_iter()
        .chain(stem)
        .chain(ep_fallback)
        .map(normalize_id_slug)
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "filter".to_string());

    let mut id = format!("adobe.8bf.{slug}");
    if id.len() > 64 {
        id.truncate(64);
        while id.ends_with(['_', '.', '-']) {
            id.pop();
        }
    }
    if !crate::manifest::valid_id(&id) {
        id = "adobe.8bf.filter".to_string();
    }

    let raw_name = if !desc.pipl.name.is_empty() {
        desc.pipl.name.as_str()
    } else if let Some(st) = stem {
        st
    } else if !desc.entrypoint_symbol.is_empty() {
        desc.entrypoint_symbol.as_str()
    } else {
        "Photoshop .8bf Filter"
    };
    let trimmed: String = trim_name(raw_name).chars().take(64).collect();
    let final_trimmed = trim_name(&trimmed);
    let name = if printable_name(final_trimmed) { final_trimmed.to_owned() } else { "Photoshop .8bf Filter".to_owned() };

    let version = if desc.pipl.version_major != 0 || desc.pipl.version_minor != 0 {
        format!("{}.{}.0", desc.pipl.version_major, desc.pipl.version_minor)
    } else {
        "1.0.0".to_string()
    };

    let mut description = if !desc.pipl.category.is_empty() {
        format!("{} ({:?} {:?} .8bf filter)", desc.pipl.category, desc.guest_os, desc.arch)
    } else {
        format!("Native {:?} {:?} .8bf filter", desc.guest_os, desc.arch)
    };
    if description.len() > 256 {
        description = description.chars().take(256).collect();
    }
    let author: String = desc.pipl.category.chars().take(64).collect();

    let man = Manifest { id, name, version, kind: Kind::Filter, description, author, params: Vec::new(), overlap: 0, area: Area::Canvas };
    man.validate()?;
    Ok(man)
}

impl Plugin {
    /// Validates and loads a plug-in module (`.wasm` or 64-bit native `.8bf` / Mach-O `.plugin`).
    pub fn load(bytes: &[u8], limits: Limits) -> Result<Plugin> {
        Self::load_with_hint(bytes, None, limits)
    }

    /// Validates and loads a plug-in module with an optional filename hint (`"Solarize.8bf"`,
    /// `"SwapRB.plugin"`, `"invert.wasm"`).
    pub fn load_with_hint(bytes: &[u8], file_hint: Option<&str>, limits: Limits) -> Result<Plugin> {
        if bytes.len() > limits.max_module_bytes {
            return Err(Error::Module(format!("module is {} bytes (limit {})", bytes.len(), limits.max_module_bytes)));
        }
        if bytes.starts_with(b"\0asm") {
            return Self::load_wasm(bytes, limits);
        }
        if crate::native_vm::pe::is_pe_binary(bytes) || crate::native_vm::macho::is_macho_or_fat(bytes) {
            return Self::load_native_8bf(bytes, file_hint, limits);
        }
        let msg = "not a WebAssembly binary (missing \\0asm header)";
        Err(Error::Module(match format_hint(bytes) {
            Some(hint) => format!("{msg}; this file looks like {hint} - PhotoCraft plug-ins are WebAssembly modules (.wasm)"),
            None => msg.to_string(),
        }))
    }

    fn load_wasm(bytes: &[u8], limits: Limits) -> Result<Plugin> {
        check_code_section(bytes)?;
        let mut config = Config::default();
        config.consume_fuel(true).enforced_limits(EnforcedLimits::strict()).set_max_recursion_depth(limits.max_recursion_depth.max(16));
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).map_err(|e| Error::Module(e.to_string()))?;
        if let Some(imp) = module.imports().next() {
            return Err(Error::Abi(format!(
                "the module imports `{}.{}`, but plug-ins get no host imports (no WASI, files or network)",
                imp.module(),
                imp.name()
            )));
        }
        let mut p = Plugin { manifest: placeholder_manifest(), backend: Backend::Wasm { engine, module }, limits, size: bytes.len(), source: None };
        let deadline = Deadline::new(&p.limits);
        let mut live = p.instantiate(&deadline)?;
        let version: TypedFunc<(), i32> = live.func("pc_abi_version")?;
        let v = p.call(&mut live, &version, (), p.limits.fuel_base, &deadline)?;
        if v != ABI_VERSION {
            return Err(Error::Abi(format!("pc_abi_version returned {v}; this host implements version {ABI_VERSION}")));
        }
        let manifest: TypedFunc<(), i64> = live.func("pc_manifest")?;
        let packed = p.call(&mut live, &manifest, (), p.limits.fuel_base, &deadline)? as u64;
        let (ptr, len) = ((packed & 0xffff_ffff) as usize, (packed >> 32) as usize);
        if len > crate::manifest::MAX_MANIFEST_BYTES {
            return Err(Error::Manifest(format!("manifest is {len} bytes (limit {})", crate::manifest::MAX_MANIFEST_BYTES)));
        }
        let text = live.memory.data(&live.store).get(ptr..ptr.saturating_add(len)).ok_or_else(|| Error::Abi("pc_manifest points outside memory".into()))?;
        p.manifest = Manifest::parse(text)?;
        // The other exports must exist with the right types.
        live.func::<i32, i32>("pc_alloc")?;
        live.func::<FilterArgs, i32>("pc_filter")?;
        Ok(p)
    }

    fn load_native_8bf(bytes: &[u8], file_hint: Option<&str>, limits: Limits) -> Result<Plugin> {
        if let Some(h) = file_hint {
            let lower = h.trim().to_ascii_lowercase();
            if lower.ends_with(".8bt") || lower.ends_with(".aex") || lower.ends_with(".prm") || lower.ends_with(".aip") {
                return Err(Error::Module("unsupported native plug-in extension (expected a Photoshop .8bf/.plugin filter or .wasm module)".into()));
            }
        }
        let vm_limits = crate::native_vm::Limits {
            max_binary_bytes: limits.max_module_bytes,
            max_memory_bytes: limits.max_memory_bytes,
            instruction_budget: limits.fuel_base,
            max_call_depth: limits.max_recursion_depth.clamp(16, 512),
        };
        let host = Filter8bfHost::from_bytes(bytes.to_vec(), file_hint, vm_limits).map_err(native_err)?;
        if matches!(host.descriptor().pipl.format_hint, Some(PluginFormat::AfterEffectsAex | PluginFormat::PremierePrm | PluginFormat::IllustratorAip)) {
            return Err(Error::Abi(format!("plug-in kind {:?} is not a Photoshop .8bf filter", host.descriptor().pipl.kind_4cc)));
        }
        let manifest = synthesize_8bf_manifest(host.descriptor(), file_hint)?;
        Ok(Plugin { manifest, backend: Backend::Native8bf(Box::new(host)), limits, size: bytes.len(), source: None })
    }

    /// Returns `true` when this plug-in is a native C++ `.8bf` / Mach-O `.plugin` filter running in `native_vm`.
    pub fn is_native_8bf(&self) -> bool {
        matches!(self.backend, Backend::Native8bf(_))
    }

    /// Static binary descriptor (`PiPL`, guest OS, CPU ISA, entrypoint) when backed by `native_vm`.
    pub fn native_descriptor(&self) -> Option<&PluginDescriptor> {
        match &self.backend {
            Backend::Native8bf(host) => Some(host.descriptor()),
            Backend::Wasm { .. } => None,
        }
    }

    pub(crate) fn native_8bf_host(&self) -> Option<&Filter8bfHost> {
        match &self.backend {
            Backend::Native8bf(host) => Some(host),
            Backend::Wasm { .. } => None,
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Module size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
    /// Where the module was loaded from, if it came from a file.
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub(crate) fn instantiate(&self, deadline: &Deadline) -> Result<Live> {
        deadline.check()?;
        let Backend::Wasm { engine, module } = &self.backend else {
            return Err(Error::Abi("native .8bf plug-ins do not instantiate a wasmi store".into()));
        };
        let limits = StoreLimitsBuilder::new().memory_size(self.limits.max_memory_bytes).memories(1).tables(4).table_elements(100_000).instances(1).build();
        let mut store = Store::new(engine, limits);
        store.limiter(|l| l);
        // The start function (if any) runs on this fuel. It cannot be interrupted to check the
        // deadline the way a call can, so it gets no more than one slice, however much a call may use.
        store.set_fuel(self.limits.fuel_base.min(FUEL_SLICE)).map_err(wasm_err)?;
        let linker = Linker::<StoreLimits>::new(engine);
        let instance = linker.instantiate_and_start(&mut store, module).map_err(wasm_err)?;
        let memory = instance.get_memory(&store, "memory").ok_or_else(|| Error::Abi("the module must export its linear memory as `memory`".into()))?;
        Ok(Live { store, instance, memory })
    }

    /// Calls `f` with at most `fuel` instructions, in slices, checking the deadline between them.
    pub(crate) fn call<P: WasmParams, R: WasmResults>(&self, live: &mut Live, f: &TypedFunc<P, R>, params: P, fuel: u64, deadline: &Deadline) -> Result<R> {
        let mut left = fuel;
        let give = left.min(FUEL_SLICE);
        left -= give;
        live.store.set_fuel(give).map_err(wasm_err)?;
        let mut call = f.call_resumable(&mut live.store, params).map_err(wasm_err)?;
        loop {
            match call {
                TypedResumableCall::Finished(r) => return Ok(r),
                TypedResumableCall::HostTrap(_) => return Err(Error::Trap("unexpected host trap".into())),
                TypedResumableCall::OutOfFuel(inv) => {
                    let need = inv.required_fuel();
                    if left == 0 || need > left {
                        return Err(Error::Limit("instruction budget".into()));
                    }
                    deadline.check()?;
                    let give = left.min(FUEL_SLICE.max(need));
                    left -= give;
                    live.store.set_fuel(give).map_err(wasm_err)?;
                    call = inv.resume(&mut live.store).map_err(wasm_err)?;
                }
            }
        }
    }

    /// Runs `pc_filter` on `data` (interleaved `f32`, `w × h × channels`) in a fresh instance.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn filter_buffer(&self, data: &mut [f32], w: u32, h: u32, channels: u32, format: u32, params: &[u8], deadline: &Deadline) -> Result<()> {
        let bytes = data.len().checked_mul(4).filter(|b| *b <= i32::MAX as usize && *b + params.len() <= self.limits.max_memory_bytes);
        let bytes = bytes.ok_or_else(|| Error::Limit("memory budget (band too large)".into()))?;
        let mut live = self.instantiate(deadline)?;
        let alloc: TypedFunc<i32, i32> = live.func("pc_alloc")?;
        let filter: TypedFunc<FilterArgs, i32> = live.func("pc_filter")?;
        let ptr = self.call(&mut live, &alloc, bytes as i32, self.limits.fuel_base, deadline)? as u32 as usize;
        let pptr = self.call(&mut live, &alloc, params.len().max(1) as i32, self.limits.fuel_base, deadline)? as u32 as usize;
        if ptr == 0 || pptr == 0 {
            return Err(Error::Failed("pc_alloc returned 0 (out of memory)".into()));
        }
        let outside = || Error::Abi("pc_alloc returned a block outside memory".into());
        let end = ptr.checked_add(bytes).ok_or_else(outside)?;
        let pend = pptr.checked_add(params.len()).ok_or_else(outside)?;
        {
            let mem = live.memory.data_mut(&mut live.store);
            let dst = mem.get_mut(ptr..end).ok_or_else(outside)?;
            for (d, v) in dst.as_chunks_mut::<4>().0.iter_mut().zip(data.iter()) {
                *d = v.to_le_bytes();
            }
            let pdst = mem.get_mut(pptr..pend).ok_or_else(outside)?;
            pdst.copy_from_slice(params);
        }
        let fuel = self.limits.fuel_base.saturating_add((data.len() as u64).saturating_mul(self.limits.fuel_per_sample));
        let args = (ptr as i32, bytes as i32, w as i32, h as i32, channels as i32, format as i32, pptr as i32, params.len() as i32);
        let rc = self.call(&mut live, &filter, args, fuel, deadline)?;
        if rc != 0 {
            return Err(Error::Failed(format!("pc_filter returned error code {rc}")));
        }
        let mem = live.memory.data(&live.store);
        let src = mem.get(ptr..end).ok_or_else(|| Error::Abi("pixel buffer no longer in memory".into()))?;
        for (v, s) in data.iter_mut().zip(src.as_chunks::<4>().0) {
            *v = f32::from_le_bytes(*s);
        }
        Ok(())
    }
}

/// `pc_filter(buf_ptr, buf_len, width, height, channels, format, params_ptr, params_len)`.
type FilterArgs = (i32, i32, i32, i32, i32, i32, i32, i32);

impl Live {
    fn func<P: WasmParams, R: WasmResults>(&self, name: &str) -> Result<TypedFunc<P, R>> {
        self.instance.get_typed_func::<P, R>(&self.store, name).map_err(|e| Error::Abi(format!("export `{name}`: {e}")))
    }
}

fn placeholder_manifest() -> Manifest {
    Manifest {
        id: String::new(),
        name: String::new(),
        version: String::new(),
        kind: crate::Kind::Filter,
        description: String::new(),
        author: String::new(),
        params: Vec::new(),
        overlap: 0,
        area: crate::Area::Content,
    }
}
