//! The plug-in runtime: loading, instantiating and calling sandboxed WebAssembly (`.wasm`)
//! modules in [`wasmi`] and compiled C++ Adobe Illustrator (`.aip` / `.bundle`) SweetPea/PICA
//! binaries in [`crate::native_vm`] under resource limits.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_geom::{Anchor, AnchorKind, PathData, Point, SubPath};
use wasmi::{
    Config, CustomFuelCosts, EnforcedLimits, Engine, Instance, Memory, Module, Store, StoreLimitsBuilder, TypedFunc, TypedResumableCall, Val,
    WasmParams, WasmResults,
};

use crate::host::{self, Bridge, Deadline, Host, Report, Run};
use crate::manifest::{MAX_MANIFEST_BYTES, Manifest, valid_id};
use crate::native_vm::{AipBezierSegment, AipHost, AipRegistrationProbe, AipSubpath, PluginDescriptor, PluginHostError};
use crate::permissions::Permission;
use crate::{Error, Kind, Result};

/// The plug-in ABI version this host implements (`vc_abi_version` must return it).
pub const ABI_VERSION: i32 = 1;

/// Fuel handed out per slice; between slices the host checks the deadline.
pub const FUEL_SLICE: u64 = 20_000_000;

/// wasmi's fuel costs, but compiling costs nothing. wasmi compiles a function when it is first
/// called, and charges the run that calls it by the function's size. When the slice a run was in
/// ran out meanwhile, wasmi ended the run with an error, not an out-of-fuel pause the host resumes:
/// a first run failed with `Limit("instruction budget")` and most of its budget left, as where the
/// slices fell decided. So a run pays for its own instructions only, as many on a first run as on
/// the next. Copying keeps wasmi's default cost, a unit for every 64 bytes.
const FUEL_COSTS: CustomFuelCosts = CustomFuelCosts { bytes_copied_per_fuel: 64, fuel_per_bytes_translated: 0, fuel_per_bytes_validated: 0 };

/// Bytes a bulk memory instruction copies or fills for each unit of fuel.
pub const BYTES_COPIED_PER_FUEL: u32 = FUEL_COSTS.bytes_copied_per_fuel;

/// Instructions a heavy filter's run may use besides [`Limits::fuel_per_byte`] for each byte of
/// input ([`crate::Budget::Heavy`]): 20 times a light one's. Its output's size can't add to it:
/// the host learns that only once the run has ended.
pub const HEAVY_FUEL: u64 = 1_000_000_000;

/// Wall-clock budget of a heavy filter's run, in milliseconds.
pub const HEAVY_WALL_TIME_MS: u64 = 300_000;

/// The limit a run reports when its input and parameters together are past half its memory.
pub(crate) const SELECTION_TOO_LARGE: &str = "memory budget (the selection is too large to hand over)";

/// A heavy run's budget as people read it: (minutes it may take, times the instructions a light
/// run gets).
pub fn heavy_terms() -> (u64, u64) {
    (HEAVY_WALL_TIME_MS / 60_000, HEAVY_FUEL / Limits::default().fuel_base.max(1))
}

/// Whether `bytes` has a 64-bit PE32+ or Mach-O / Universal Fat binary header supported by the
/// zero-JIT `.aip` C++ host.
pub fn is_native_plugin_bytes(bytes: &[u8]) -> bool {
    crate::native_vm::pe::is_pe_binary(bytes) || crate::native_vm::macho::is_macho_or_fat(bytes)
}

/// Resource limits for running plug-ins.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// Largest module accepted.
    pub max_module_bytes: usize,
    /// Cap on a plug-in's linear memory. The input and the parameters may use half of it.
    pub max_memory_bytes: usize,
    /// Largest output JSON read back.
    pub max_output_bytes: usize,
    /// Instructions (fuel) any call may use, plus [`Limits::fuel_per_byte`] per byte of input for
    /// `vc_run`.
    pub fuel_base: u64,
    pub fuel_per_byte: u64,
    /// Deepest call nesting.
    pub max_recursion_depth: usize,
    /// Wall-clock budget for one run, in milliseconds. The web build measures it with the page's
    /// `performance.now()`.
    pub wall_time_ms: u64,
    /// The most a heavy filter ([`crate::Budget::Heavy`]) is given instead of `fuel_base` and
    /// `wall_time_ms`: what the host allows, which the module's manifest can ask for but never
    /// raise. Set them to `fuel_base` and `wall_time_ms` to give heavy filters nothing more.
    pub heavy_fuel: u64,
    pub heavy_wall_time_ms: u64,
    /// What one export or import is given instead of `fuel_base` and `wall_time_ms`
    /// ([`crate::export::limits`]).
    pub export_fuel: u64,
    pub export_wall_time_ms: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_module_bytes: 32 << 20,
            max_memory_bytes: 512 << 20,
            max_output_bytes: 64 << 20,
            fuel_base: 50_000_000,
            fuel_per_byte: 4_000,
            max_recursion_depth: 1024,
            wall_time_ms: 60_000,
            heavy_fuel: HEAVY_FUEL,
            heavy_wall_time_ms: HEAVY_WALL_TIME_MS,
            export_fuel: crate::export::FUEL,
            export_wall_time_ms: crate::export::WALL_TIME_MS,
        }
    }
}

enum RuntimeBackend {
    Wasm { engine: Engine, module: Module },
    NativeAip { host: Arc<AipHost>, ext: &'static str },
}

/// A loaded, validated plug-in (`.wasm` or compiled C++ `.aip` / `.bundle`). Cheap to share (`Arc`)
/// and safe to run from several threads (`Send + Sync`): every run gets its own isolated store/VM.
pub struct Plugin {
    manifest: Manifest,
    backend: RuntimeBackend,
    limits: Limits,
    size: usize,
    hash: String,
    source: Option<String>,
    serial: u64,
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin").field("manifest", &self.manifest).field("size", &self.size).field("source", &self.source).finish()
    }
}

/// One live instance: a store with its own memory limit and the run's state, and the module's
/// exports.
struct Live {
    store: Store<Host>,
    instance: Instance,
    memory: Memory,
}

impl Live {
    fn func<P: WasmParams, R: WasmResults>(&self, name: &str) -> Result<TypedFunc<P, R>> {
        self.instance.get_typed_func::<P, R>(&self.store, name).map_err(|e| Error::Abi(format!("export `{name}`: {e}")))
    }

    /// The `len << 32 | ptr` block a plug-in returned, copied out (at most `max` bytes).
    fn read_packed(&self, packed: i64, what: &str, max: usize) -> Result<Vec<u8>> {
        let packed = packed as u64;
        let (ptr, len) = ((packed & 0xffff_ffff) as usize, (packed >> 32) as usize);
        if len > max {
            return Err(Error::Limit(format!("output size ({what} is {len} bytes, limit {max})")));
        }
        let bytes = self.memory.data(&self.store).get(ptr..ptr.saturating_add(len));
        bytes.map(<[u8]>::to_vec).ok_or_else(|| Error::Abi(format!("{what} points outside memory")))
    }
}

/// Takes `cost` instructions from a run: from the budget still to hand out (`left`), then from what
/// the store holds. A run that can't pay ends with its instruction budget.
fn charge(live: &mut Live, left: &mut u64, cost: u64) -> Result<()> {
    let held = live.store.get_fuel().map_err(wasm_err)?;
    if held.saturating_add(*left) < cost {
        return Err(Error::Limit("instruction budget".into()));
    }
    let from_left = cost.min(*left);
    *left -= from_left;
    live.store.set_fuel(held - (cost - from_left)).map_err(wasm_err)
}

fn wasm_err(e: wasmi::Error) -> Error {
    if e.as_trap_code() == Some(wasmi::TrapCode::OutOfFuel) { Error::Limit("instruction budget".into()) } else { Error::Trap(e.to_string()) }
}

fn map_vm_err(e: PluginHostError) -> Error {
    let detail = e.to_string();
    match e {
        PluginHostError::InvalidBinary(_) | PluginHostError::UnsupportedArch(_) | PluginHostError::MissingEntrypoint(_) => Error::Module(detail),
        PluginHostError::Limit(_) => Error::Limit("instruction budget".into()),
        PluginHostError::Abi(_) => Error::Abi(detail),
        PluginHostError::MemoryFault(_) | PluginHostError::CpuTrap(_) => Error::Trap(detail),
        PluginHostError::PluginError(_) => Error::Failed(detail),
    }
}

/// `vc_run(input_ptr, input_len, params_ptr, params_len) -> len << 32 | ptr` (negative: error).
type RunArgs = (i32, i32, i32, i32);

impl Plugin {
    /// Validates and loads a plug-in module (`.wasm` WebAssembly module or 64-bit PE32+ / Mach-O
    /// `.aip` / `.bundle` C++ binary). In Safe Mode nothing is loaded ([`Error::SafeMode`]).
    pub fn load(bytes: &[u8], limits: Limits) -> Result<Plugin> {
        Self::load_with_hint(bytes, None, limits)
    }

    /// [`Plugin::load`] with an optional filename or path hint (`"AddAnchor.aip"`, `"Round.bundle"`)
    /// used as fallback metadata when a compiled `.aip` binary has no embedded `PiPL` name.
    pub fn load_with_hint(bytes: &[u8], file_hint: Option<&str>, limits: Limits) -> Result<Plugin> {
        crate::safe_mode::check()?;
        if bytes.len() > limits.max_module_bytes {
            return Err(Error::Module(format!("module is {} bytes (limit {})", bytes.len(), limits.max_module_bytes)));
        }
        if bytes.starts_with(b"\0asm") {
            return Self::load_wasm(bytes, limits);
        }
        if is_native_plugin_bytes(bytes) {
            return Self::load_native_aip(bytes, file_hint, limits);
        }
        Err(Error::Module("not a WebAssembly binary (missing \\0asm header)".into()))
    }

    fn load_wasm(bytes: &[u8], limits: Limits) -> Result<Plugin> {
        let mut config = Config::default();
        config
            .consume_fuel(true)
            .fuel_cost(FUEL_COSTS)
            .enforced_limits(EnforcedLimits::strict())
            .set_max_recursion_depth(limits.max_recursion_depth.max(16));
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).map_err(|e| Error::Module(e.to_string()))?;
        let imports = host::check_imports(&module)?;
        let hash = crate::permissions::sha256_hex(bytes);
        let mut p = Plugin {
            manifest: Manifest::placeholder(),
            backend: RuntimeBackend::Wasm { engine, module },
            limits,
            size: bytes.len(),
            hash,
            source: None,
            serial: 0,
        };
        let deadline = Deadline::new(p.limits.wall_time_ms);
        // The manifest isn't read yet: every host import is linked, what it reports is dropped and
        // there is no storage to reach.
        let mut live = p.instantiate(Run::new("", false, false, deadline), &crate::permissions::ALL, &mut Report::default())?;
        let version: TypedFunc<(), i32> = live.func("vc_abi_version")?;
        let v = p.call(&mut live, &version, (), p.limits.fuel_base, &deadline)?;
        if v != ABI_VERSION {
            return Err(Error::Abi(format!("vc_abi_version returned {v}; this host implements version {ABI_VERSION}")));
        }
        let manifest: TypedFunc<(), i64> = live.func("vc_manifest")?;
        let packed = p.call(&mut live, &manifest, (), p.limits.fuel_base, &deadline)?;
        let text = live.read_packed(packed, "vc_manifest", MAX_MANIFEST_BYTES).map_err(|e| match e {
            Error::Limit(_) => Error::Manifest(format!("manifest is too long (limit {MAX_MANIFEST_BYTES} bytes)")),
            e => e,
        })?;
        p.manifest = Manifest::parse(&text)?;
        host::check_granted(&p.manifest, &imports)?;
        // A heavy filter's runs get the larger budget the host allows for one, which the host
        // lets the user allow or refuse before installing it.
        if p.manifest.budget == crate::Budget::Heavy {
            p.limits.fuel_base = p.limits.fuel_base.max(p.limits.heavy_fuel);
            p.limits.wall_time_ms = p.limits.wall_time_ms.max(p.limits.heavy_wall_time_ms);
        }
        // The other exports must exist with the right types.
        live.func::<i32, i32>("vc_alloc")?;
        live.func::<RunArgs, i64>("vc_run")?;
        Ok(p)
    }

    fn load_native_aip(bytes: &[u8], file_hint: Option<&str>, limits: Limits) -> Result<Plugin> {
        let vm_limits = crate::native_vm::Limits {
            max_binary_bytes: limits.max_module_bytes,
            max_memory_bytes: limits.max_memory_bytes,
            instruction_budget: limits.fuel_base,
            max_call_depth: limits.max_recursion_depth.clamp(16, 512),
        };
        let host = AipHost::from_bytes(bytes.to_vec(), file_hint, vm_limits).map_err(map_vm_err)?;
        let probe = host.probe_registration().unwrap_or_default();
        let manifest = synthesize_aip_manifest(host.descriptor(), &probe, file_hint)?;
        let ext = if file_hint.is_some_and(|h| h.to_ascii_lowercase().ends_with(".bundle")) { "bundle" } else { "aip" };
        let hash = crate::permissions::sha256_hex(bytes);
        Ok(Plugin {
            manifest,
            backend: RuntimeBackend::NativeAip { host: Arc::new(host), ext },
            limits,
            size: bytes.len(),
            hash,
            source: None,
            serial: 0,
        })
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
    /// The module's SHA-256, in lower-case hexadecimal.
    pub fn hash(&self) -> &str {
        &self.hash
    }
    /// Whether this plug-in is a compiled C++ Illustrator `.aip` / `.bundle` binary.
    pub fn is_native_aip(&self) -> bool {
        matches!(self.backend, RuntimeBackend::NativeAip { .. })
    }
    /// Static binary descriptor when loaded from a compiled `.aip` / `.bundle` binary.
    pub fn aip_descriptor(&self) -> Option<&PluginDescriptor> {
        match &self.backend {
            RuntimeBackend::NativeAip { host, .. } => Some(host.descriptor()),
            RuntimeBackend::Wasm { .. } => None,
        }
    }
    /// Canonical file extension (`"wasm"`, `"aip"`, or `"bundle"`) for storing this plug-in on disk.
    pub fn file_extension(&self) -> &'static str {
        match &self.backend {
            RuntimeBackend::Wasm { .. } => "wasm",
            RuntimeBackend::NativeAip { ext, .. } => {
                if self.source.as_deref().is_some_and(|s| s.to_ascii_lowercase().ends_with(".bundle")) {
                    "bundle"
                } else {
                    ext
                }
            }
        }
    }
    /// Where the module was loaded from, if it came from a file.
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
    /// A number unique to this installation (re-installing a plug-in gives it a new one), so
    /// caches of its results can tell versions apart. 0 until installed.
    pub fn serial(&self) -> u64 {
        self.serial
    }
    pub(crate) fn set_serial(&mut self, serial: u64) {
        self.serial = serial;
    }

    /// Runs an object filter pass over `input` (`{"objects": [...]}`) and returns the transformed
    /// output JSON bytes (works for both `.wasm` and native `.aip` plug-ins).
    pub fn run_filter(&self, input: &[u8], params: &[u8]) -> Result<Vec<u8>> {
        self.run(input, params)
    }

    /// Runs a live-effect pass over `input` (`{"objects": [...]}`) under `limits` and returns the
    /// transformed output JSON bytes (works for both `.wasm` and native `.aip` plug-ins).
    pub fn run_effect(&self, input: &[u8], params: &[u8], limits: &Limits) -> Result<Vec<u8>> {
        self.run_limited(input, params, limits)
    }

    /// A new instance for `run`, with the host imports of the permissions `granted` linked. When the
    /// module's start function fails, what it reported through the host imports goes to `report`.
    fn instantiate(&self, run: Run, granted: &[Permission], report: &mut Report) -> Result<Live> {
        let RuntimeBackend::Wasm { engine, module } = &self.backend else {
            return Err(Error::Trap("unexpected host trap".into()));
        };
        let limits =
            StoreLimitsBuilder::new().memory_size(self.limits.max_memory_bytes).memories(1).tables(4).table_elements(100_000).instances(1).build();
        let mut store = Store::new(engine, Host { limits, run });
        store.limiter(|h| &mut h.limits);
        // The start function (if any) runs on one slice of fuel: it runs before `call` can check
        // the deadline or a cancel between slices, so a heavy budget would let it run for minutes.
        store.set_fuel(self.limits.fuel_base.min(FUEL_SLICE)).map_err(wasm_err)?;
        let linker = host::linker(engine, granted);
        let instance = match linker.instantiate_and_start(&mut store, module) {
            Ok(instance) => instance,
            Err(e) => {
                *report = std::mem::take(&mut store.data_mut().run.report);
                return Err(wasm_err(e));
            }
        };
        let memory =
            instance.get_memory(&store, "memory").ok_or_else(|| Error::Abi("the module must export its linear memory as `memory`".into()))?;
        Ok(Live { store, instance, memory })
    }

    /// Calls `f` with at most `fuel` instructions, in slices, checking the deadline and the
    /// caller's cancellation ([`vectorcraft_doc::cancel`]) between them.
    ///
    /// wasmi pauses before an instruction that the store's fuel can't pay for, and charges nothing
    /// for it. It charges a `memory.fill` or `memory.copy` for its whole length before writing, so
    /// up to that cost can be left in the store at a pause. Each slice tops the store up to the
    /// slice size, keeping that fuel, and a run ends with the instruction budget only when the
    /// store's fuel and the rest of the budget together can't pay for the next instruction.
    fn call<P: WasmParams, R: WasmResults>(&self, live: &mut Live, f: &TypedFunc<P, R>, params: P, fuel: u64, deadline: &Deadline) -> Result<R> {
        self.call_with(live, f, params, fuel, deadline, None)
    }

    /// [`Plugin::call`], making each command call a `vc_call` pauses it for with `bridge`
    /// ([`host::Run::pending`]) and resuming it with the code. Each call costs [`host::CALL_FUEL`]
    /// instructions and one for every 64 bytes of its arguments and result; the deadline and the
    /// caller's cancellation are checked before and after it.
    fn call_with<P: WasmParams, R: WasmResults>(
        &self,
        live: &mut Live,
        f: &TypedFunc<P, R>,
        params: P,
        fuel: u64,
        deadline: &Deadline,
        mut bridge: Option<&mut Bridge<'_>>,
    ) -> Result<R> {
        let mut left = fuel;
        let give = left.min(FUEL_SLICE);
        left -= give;
        live.store.set_fuel(give).map_err(wasm_err)?;
        let mut call = f.call_resumable(&mut live.store, params).map_err(wasm_err)?;
        loop {
            match call {
                TypedResumableCall::Finished(r) => return Ok(r),
                TypedResumableCall::HostTrap(inv) => {
                    let pending = live.store.data_mut().run.pending.take();
                    let (Some((id, args, size)), Some(bridge)) = (pending, bridge.as_deref_mut()) else {
                        return Err(Error::Trap("unexpected host trap".into()));
                    };
                    charge(live, &mut left, host::CALL_FUEL.saturating_add(size as u64 / 64))?;
                    deadline.check()?;
                    vectorcraft_doc::cancel::check().map_err(|_| Error::Cancelled)?;
                    let answer = bridge(&id, &args);
                    let code = live.store.data_mut().run.call_answered(&id, answer);
                    let returned = live.store.data().run.call_result.as_ref().map_or(0, Vec::len);
                    charge(live, &mut left, returned as u64 / 64)?;
                    deadline.check()?;
                    vectorcraft_doc::cancel::check().map_err(|_| Error::Cancelled)?;
                    call = inv.resume(&mut live.store, &[Val::I32(code)]).map_err(wasm_err)?;
                }
                TypedResumableCall::OutOfFuel(inv) => {
                    let need = inv.required_fuel();
                    let held = live.store.get_fuel().map_err(wasm_err)?;
                    let total = held.saturating_add(left);
                    if need > total {
                        return Err(Error::Limit("instruction budget".into()));
                    }
                    deadline.check()?;
                    vectorcraft_doc::cancel::check().map_err(|_| Error::Cancelled)?;
                    let fill = total.min(FUEL_SLICE.max(need)).max(held);
                    left -= fill - held;
                    live.store.set_fuel(fill).map_err(wasm_err)?;
                    call = inv.resume(&mut live.store).map_err(wasm_err)?;
                }
            }
        }
    }

    /// Runs `vc_run` on `input` (the objects JSON) with `params` (the parameters JSON) in a fresh
    /// instance and returns the output JSON bytes.
    pub fn run(&self, input: &[u8], params: &[u8]) -> Result<Vec<u8>> {
        self.run_limited(input, params, &self.limits)
    }

    /// [`Plugin::run`] with other time and instruction budgets (live effects get a shorter one). In
    /// Safe Mode nothing runs ([`Error::SafeMode`]), even a plug-in installed before it was on.
    pub fn run_limited(&self, input: &[u8], params: &[u8], limits: &Limits) -> Result<Vec<u8>> {
        self.run_reporting(input, params, limits).0
    }

    /// [`Plugin::run_limited`], and what the run reported through the host imports: its last
    /// progress report and its log lines, which also go to the app's log under its id. Its storage
    /// imports reach the storage of its id ([`crate::registry::storage`]). Inside a
    /// [`vectorcraft_doc::cancel::scope`] its progress goes to the scope's sink (a live effect's
    /// doesn't) and `vc_cancelled` sees the scope's Cancel.
    pub fn run_reporting(&self, input: &[u8], params: &[u8], limits: &Limits) -> (Result<Vec<u8>>, Report) {
        self.run_guarded(input, params, limits, None)
    }

    /// [`Plugin::run_reporting`] for a filter whose manifest's `commandAllowlist` names commands:
    /// each `vc_call` of one of them is made with `bridge`, on the caller's thread, while the run
    /// waits. A failed call fails the run, also when the plug-in goes on and returns an output; the
    /// error names the command.
    pub fn run_calling(&self, input: &[u8], params: &[u8], limits: &Limits, bridge: &mut Bridge<'_>) -> (Result<Vec<u8>>, Report) {
        self.run_guarded(input, params, limits, Some(bridge))
    }

    fn run_guarded(&self, input: &[u8], params: &[u8], limits: &Limits, bridge: Option<&mut Bridge<'_>>) -> (Result<Vec<u8>>, Report) {
        let mut report = Report::default();
        if let Err(e) = crate::safe_mode::check() {
            return (Err(e), report);
        }
        // wasmi reports misbehaving modules as errors; a panic would be a bug in it, and must
        // still not take the document down (native builds; wasm aborts on panic anyway).
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.run_inner(input, params, limits, &mut report, bridge)));
        (run.map_err(|_| Error::Trap("the runtime panicked".into())).and_then(|r| r), report)
    }

    fn run_inner(&self, input: &[u8], params: &[u8], limits: &Limits, report: &mut Report, bridge: Option<&mut Bridge<'_>>) -> Result<Vec<u8>> {
        let total = input.len().checked_add(params.len()).filter(|b| *b <= i32::MAX as usize && *b <= self.limits.max_memory_bytes / 2);
        total.ok_or_else(|| Error::Limit(SELECTION_TOO_LARGE.into()))?;
        let deadline = Deadline::new(limits.wall_time_ms);
        if let RuntimeBackend::NativeAip { host, .. } = &self.backend {
            return run_native_aip(host, input, params, limits, &deadline);
        }
        let m = &self.manifest;
        let run = Run::new(&m.id, m.kind != Kind::Effect, true, deadline).with_storage(crate::registry::storage(&m.id));
        let mut live = self.instantiate(run, &m.permissions(), report)?;
        // Calls can be made from `vc_run` on, not from the start function.
        if bridge.is_some() {
            live.store.data_mut().run.allow_calls(m.command_allowlist.clone());
        }
        let out = self.run_live(&mut live, input, params, limits, &deadline, bridge);
        let state = &mut live.store.data_mut().run;
        *report = std::mem::take(&mut state.report);
        // A failed call fails the run, whatever the plug-in did after it, unless the run was
        // canceled.
        match (state.call_failure.take(), out) {
            (_, Err(Error::Cancelled)) => Err(Error::Cancelled),
            (Some(e), _) => Err(e),
            (None, out) => out,
        }
    }

    /// Hands `input` and `params` to the instance `live` and runs `vc_run` → the output JSON bytes.
    fn run_live(
        &self,
        live: &mut Live,
        input: &[u8],
        params: &[u8],
        limits: &Limits,
        deadline: &Deadline,
        bridge: Option<&mut Bridge<'_>>,
    ) -> Result<Vec<u8>> {
        let alloc: TypedFunc<i32, i32> = live.func("vc_alloc")?;
        let run: TypedFunc<RunArgs, i64> = live.func("vc_run")?;
        let place = |live: &mut Live, bytes: &[u8]| -> Result<usize> {
            let ptr = self.call(live, &alloc, bytes.len().max(1) as i32, limits.fuel_base, deadline)? as u32 as usize;
            if ptr == 0 {
                return Err(Error::Failed("vc_alloc returned 0 (out of memory)".into()));
            }
            let outside = || Error::Abi("vc_alloc returned a block outside memory".into());
            let end = ptr.checked_add(bytes.len()).ok_or_else(outside)?;
            live.memory.data_mut(&mut live.store).get_mut(ptr..end).ok_or_else(outside)?.copy_from_slice(bytes);
            Ok(ptr)
        };
        let iptr = place(live, input)?;
        let pptr = place(live, params)?;
        let fuel = limits.fuel_base.saturating_add((input.len() as u64).saturating_mul(limits.fuel_per_byte));
        let packed = self.call_with(live, &run, (iptr as i32, input.len() as i32, pptr as i32, params.len() as i32), fuel, deadline, bridge)?;
        // A cancelled run, or one past its time budget, changes nothing, also when the plug-in saw
        // `vc_cancelled` and returned.
        vectorcraft_doc::cancel::check().map_err(|_| Error::Cancelled)?;
        deadline.check()?;
        if packed < 0 {
            return Err(Error::Failed(format!("vc_run returned error code {packed}")));
        }
        live.read_packed(packed, "the output", limits.max_output_bytes)
    }
}

fn hint_stem(file_hint: Option<&str>) -> Option<String> {
    let raw = file_hint?.trim();
    if raw.is_empty() {
        return None;
    }
    for part in raw.rsplit(['/', '\\']) {
        let lower = part.to_ascii_lowercase();
        if let Some(s) = lower.strip_suffix(".aip").or_else(|| lower.strip_suffix(".bundle")) {
            let orig_len = part.len().saturating_sub(part.len() - s.len());
            let stem = part.get(..orig_len).unwrap_or(part).trim();
            if !stem.is_empty() {
                return Some(stem.to_string());
            }
        }
    }
    let last = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let stem = last.rsplit_once('.').map_or(last, |(s, _)| s).trim();
    (!stem.is_empty()).then(|| stem.to_string())
}

fn normalize_aip_id(raw_candidates: &[&str]) -> String {
    for &cand in raw_candidates {
        let trimmed = cand.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut slug = String::new();
        let mut prev_sep = true;
        for ch in trimmed.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
                prev_sep = false;
            } else if matches!(ch, '.' | '_' | '-') {
                if !prev_sep {
                    slug.push(ch);
                    prev_sep = true;
                }
            } else if !prev_sep {
                slug.push('-');
                prev_sep = true;
            }
        }
        let clean = slug.trim_matches(['.', '_', '-']);
        if clean.is_empty() {
            continue;
        }
        let full = if clean.contains('.') { clean.to_string() } else { format!("aip.{clean}") };
        let capped: String = full.chars().take(64).collect();
        let final_id = capped.trim_end_matches(['.', '_', '-']).to_string();
        if valid_id(&final_id) {
            return final_id;
        }
    }
    "adobe.aip.plugin".to_string()
}

fn sanitize_printable(raw: &str, max_chars: usize) -> String {
    let filtered: String = raw.chars().filter(|c| !c.is_control()).collect();
    filtered.trim().chars().take(max_chars).collect()
}

fn synthesize_aip_manifest(desc: &PluginDescriptor, probe: &AipRegistrationProbe, file_hint: Option<&str>) -> Result<Manifest> {
    let stem = hint_stem(file_hint);
    let stem_ref = stem.as_deref().unwrap_or("");
    let id = normalize_aip_id(&[&desc.pipl.match_name, &desc.pipl.name, stem_ref]);

    let reg_title = probe.filters.iter().chain(probe.effects.iter()).map(|s| sanitize_printable(s, 64)).find(|s| !s.is_empty()).unwrap_or_default();
    let name = [reg_title.as_str(), desc.pipl.name.as_str(), stem_ref, desc.entrypoint_symbol.as_str(), "Illustrator Plug-in"]
        .into_iter()
        .map(|s| sanitize_printable(s, 64))
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "Illustrator Plug-in".to_string());

    let kind = if (!probe.effects.is_empty() && probe.filters.is_empty()) || desc.pipl.kind_4cc == "AILv" { Kind::Effect } else { Kind::Filter };

    let version = if desc.pipl.version_major != 0 || desc.pipl.version_minor != 0 {
        format!("{}.{}", desc.pipl.version_major, desc.pipl.version_minor)
    } else {
        String::new()
    };
    let mut author: String = sanitize_printable(&desc.pipl.category, 64);
    while author.len() > 128 {
        author.pop();
    }

    let mut manifest = Manifest::placeholder();
    manifest.api = 1;
    manifest.id = id;
    manifest.name = name;
    manifest.kind = kind;
    manifest.version = version;
    manifest.author = author;
    manifest.validate()?;
    Ok(manifest)
}

fn extract_param_scalar(params: &[u8]) -> f64 {
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(params) else {
        return 0.0;
    };
    for key in ["amount", "value", "radius", "distance", "offset"] {
        if let Some(v) = map.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()) {
            return v;
        }
    }
    map.iter().filter(|(k, _)| k.as_str() != "_context").find_map(|(_, v)| v.as_f64().filter(|x| x.is_finite())).unwrap_or(0.0)
}

fn path_to_aip_subpaths(path: &PathData) -> Vec<AipSubpath> {
    path.subpaths
        .iter()
        .map(|sp| AipSubpath {
            closed: sp.closed,
            segments: sp
                .anchors
                .iter()
                .map(|a| AipBezierSegment {
                    p: [a.p.x, a.p.y],
                    h_in: [a.h_in.x, a.h_in.y],
                    h_out: [a.h_out.x, a.h_out.y],
                    corner: a.kind == AnchorKind::Corner,
                })
                .collect(),
        })
        .collect()
}

fn aip_subpaths_to_path(subpaths: Vec<AipSubpath>) -> PathData {
    PathData::new(
        subpaths
            .into_iter()
            .filter(|sp| !sp.segments.is_empty())
            .map(|sp| SubPath {
                closed: sp.closed,
                anchors: sp
                    .segments
                    .into_iter()
                    .map(|s| Anchor {
                        p: Point::new(s.p[0], s.p[1]),
                        h_in: Point::new(s.h_in[0], s.h_in[1]),
                        h_out: Point::new(s.h_out[0], s.h_out[1]),
                        kind: if s.corner { AnchorKind::Corner } else { AnchorKind::Smooth },
                    })
                    .collect(),
            })
            .collect(),
    )
}

fn rebuild_aip_object(orig: &Value, sub_kept: Vec<AipSubpath>) -> Option<Value> {
    let new_path = aip_subpaths_to_path(sub_kept);
    if new_path.is_empty() {
        return None;
    }
    let mut map = orig.as_object().cloned().unwrap_or_default();
    map.remove("bounds");
    if new_path.subpaths.len() > 1 && map.get("type").and_then(Value::as_str) == Some("path") {
        map.insert("type".to_string(), json!("compound"));
    }
    map.insert("path".to_string(), serde_json::to_value(&new_path).unwrap_or(Value::Null));
    Some(Value::Object(map))
}

fn run_native_aip(host: &AipHost, input: &[u8], params: &[u8], limits: &Limits, deadline: &Deadline) -> Result<Vec<u8>> {
    vectorcraft_doc::cancel::check().map_err(|_| Error::Cancelled)?;
    deadline.check()?;
    let root: Value = serde_json::from_slice(input).map_err(|e| Error::Output(format!("not JSON: {e}")))?;
    let in_objects = root.get("objects").and_then(Value::as_array).ok_or_else(|| Error::Output("expected {\"objects\": [...]}".into()))?;

    let mut all_subpaths = Vec::new();
    let mut spans: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(in_objects.len());
    for obj in in_objects {
        let parsed = obj.get("path").and_then(|p| serde_json::from_value::<PathData>(p.clone()).ok()).filter(|p| !p.subpaths.is_empty());
        if let Some(path) = parsed {
            let start = all_subpaths.len();
            all_subpaths.extend(path_to_aip_subpaths(&path));
            spans.push(Some(start..all_subpaths.len()));
        } else {
            spans.push(None);
        }
    }

    let mut out_objects = Vec::with_capacity(in_objects.len());
    if all_subpaths.is_empty() {
        out_objects.extend(in_objects.iter().cloned());
    } else {
        let fuel = limits.fuel_base.saturating_add((input.len() as u64).saturating_mul(limits.fuel_per_byte));
        let (mut kept, added) =
            host.with_instruction_budget(fuel).execute_paths_detailed(&all_subpaths, extract_param_scalar(params)).map_err(map_vm_err)?;
        vectorcraft_doc::cancel::check().map_err(|_| Error::Cancelled)?;
        deadline.check()?;

        for (obj, span) in in_objects.iter().zip(spans) {
            match span {
                None => out_objects.push(obj.clone()),
                Some(range) => {
                    let sub_kept: Vec<AipSubpath> = kept.get_mut(range).into_iter().flatten().filter_map(Option::take).collect();
                    if let Some(updated) = rebuild_aip_object(obj, sub_kept) {
                        out_objects.push(updated);
                    }
                }
            }
        }
        for extra in added {
            let new_path = aip_subpaths_to_path(vec![extra]);
            if !new_path.is_empty() {
                out_objects.push(json!({"type": "path", "path": new_path}));
            }
        }
    }

    let out_bytes = serde_json::to_vec(&json!({"objects": out_objects})).map_err(|_| Error::Params("can't encode the input".into()))?;
    let max = limits.max_output_bytes;
    let len = out_bytes.len();
    if len > max {
        let what = "the output";
        return Err(Error::Limit(format!("output size ({what} is {len} bytes, limit {max})")));
    }
    Ok(out_bytes)
}
