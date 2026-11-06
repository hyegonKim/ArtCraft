//! Explicit, offline JNI runtime evidence. A snapshot is an analyst-supplied
//! observation, not a live device or a cryptographic attestation. No context is
//! inferred from a library name, an offset alone, or an arbitrary function name.
use crate::{
    cfg_tags::Procedure,
    ghidra_port::{GhidraProgram, GhidraRefType, GhidraSourceType},
    macho_objc::MachOImage,
    pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn operation(slot: u16) -> Option<&'static str> {
    match slot {
        6 => Some("FindClass"),
        14 => Some("ThrowNew"),
        229 => Some("NewDirectByteBuffer"),
        230 => Some("GetDirectBufferAddress"),
        _ => None,
    }
}
fn text<'a>(v: &'a Value, field: &str) -> Result<&'a str, String> {
    v[field]
        .as_str()
        .ok_or_else(|| format!("Missing string {field}"))
}
fn ptr(v: &Value, field: &str) -> Result<u64, String> {
    let s = text(v, field)?;
    u64::from_str_radix(s.strip_prefix("0x").ok_or("Expected 0x pointer")?, 16)
        .map_err(|_| format!("Invalid pointer {field}"))
}
fn bytes(s: &str) -> Result<Vec<u8>, String> {
    if s.len() != 32 || !s.is_ascii() {
        return Err("Expected 16 code bytes".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|n| u8::from_str_radix(&s[n..n + 2], 16).map_err(|_| "Invalid hex code".into()))
        .collect()
}
#[derive(Debug, Clone)]
struct Target {
    address: u64,
    offset: u64,
    path: String,
}
#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    terminal_slot: u16,
    entry: u64,
    branch: u64,
    env: u64,
    table: u64,
    slots: BTreeMap<u16, Target>,
    pid: u64,
    fingerprint: String,
    expected: BTreeMap<u64, u32>,
}
impl RuntimeSnapshot {
    /// Both original ELF and pulled ART bytes must match the capture hashes.
    pub fn parse(profile: &[u8], binary: &[u8], art: &[u8]) -> Result<Self, String> {
        if profile.len() > 262144 || art.len() > 64 * 1024 * 1024 {
            return Err("Runtime evidence size limit".into());
        }
        let v: Value = serde_json::from_slice(profile).map_err(|e| e.to_string())?;
        let terminal_slot = match v["schema"].as_u64() {
            Some(1) => 14,
            Some(2) if v["terminal_jni_slot"] == 229 => 229,
            _ => return Err("Unsupported JNI capture schema/terminal slot".into()),
        };
        if text(&v, "source")? != "android-art-process-private-brk"
            || text(&v, "abi")? != "arm64-v8a"
        {
            return Err("Unsupported JNI capture schema/source/ABI".into());
        }
        if text(&v, "binary_sha256")? != sha256(binary)
            || text(&v, "runtime_sha256")? != sha256(art)
        {
            return Err("JNI evidence binary/ART SHA-256 mismatch".into());
        }
        if !binary.starts_with(b"\x7fELF") || !art.starts_with(b"\x7fELF") {
            return Err("JNI context requires original Android ELF images".into());
        }
        let image = MachOImage::parse(binary)?;
        let runtime = MachOImage::parse(art)?;
        if image.selected_arch != "arm64" || runtime.selected_arch != "arm64" {
            return Err("JNI images must be ARM64".into());
        }
        let env = ptr(&v, "env")?;
        let table = ptr(&v, "table")?;
        let pid = v["pid"]
            .as_u64()
            .filter(|p| *p > 0)
            .ok_or("Invalid capture PID")?;
        let fingerprint = text(&v, "fingerprint")?.to_string();
        if fingerprint.is_empty()
            || env == 0
            || !table.is_multiple_of(8)
            || table == 0
            || ptr(&v, "x0")? != env
        {
            return Err("Invalid runtime JNIEnv observation".into());
        }
        if ptr(&v, "original_word")? != 0xd61f0060
            || ptr(&v, "restored_word")? != 0xd61f0060
            || v["table_stable"] != true
        {
            return Err("Original BR x3 or stable table not observed".into());
        }
        let target_module = &v["target_module"];
        let load_bias = ptr(target_module, "base")?;
        let entry = ptr(target_module, "offset")?;
        if load_bias.checked_add(entry) != Some(ptr(&v, "entry")?) {
            return Err("Runtime target load bias mismatch".into());
        }
        let branch = ptr(&v, "branch_pc")?
            .checked_sub(load_bias)
            .ok_or("Invalid branch load bias")?;
        let maps = v["maps"].as_array().ok_or("Missing runtime maps")?;
        if maps.len() > 64 {
            return Err("Too many runtime maps".into());
        }
        let mapped = |address: u64, size: u64, permission: u8, path: Option<&str>| -> bool {
            let address = address & 0x00ffffffffffffff; // Android top-byte-ignore heap tags.
            maps.iter().filter_map(Value::as_str).any(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                if fields.len() < 5 {
                    return false;
                }
                let Some((start, end)) = fields[0].split_once('-') else {
                    return false;
                };
                let (Ok(start), Ok(end)) =
                    (u64::from_str_radix(start, 16), u64::from_str_radix(end, 16))
                else {
                    return false;
                };
                address >= start
                    && address.checked_add(size).is_some_and(|a| a <= end)
                    && fields[1]
                        .as_bytes()
                        .get(if permission == b'x' { 2 } else { 0 })
                        == Some(&permission)
                    && path.is_none_or(|p| fields.get(5) == Some(&p))
            })
        };
        if !mapped(env, 8, b'r', None)
            || !mapped(
                table,
                if terminal_slot == 14 { 15 * 8 } else { 231 * 8 },
                b'r',
                None,
            )
            || !mapped(
                ptr(&v, "entry")?,
                4,
                b'x',
                Some(text(target_module, "path")?),
            )
            || !mapped(
                ptr(&v, "branch_pc")?,
                4,
                b'x',
                Some(text(target_module, "path")?),
            )
        {
            return Err("JNI environment/table/code not covered by runtime maps".into());
        }
        let calls = &v["calls"];
        if calls["uninstrumented"] != 0 || calls["instrumented"] != 0 || calls["trap_hits"] != 1 {
            return Err("Runtime execution/exception evidence did not pass".into());
        }
        if terminal_slot == 14 {
            if calls["exception_class"] != "java/io/IOException"
                || calls["message_verified"] != true
            {
                return Err("ThrowNew execution evidence did not pass".into());
            }
        } else {
            let capacity = calls["source_capacity"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= 1048576)
                .ok_or("Invalid source buffer capacity")?;
            let offset = calls["offset"].as_u64().ok_or("Invalid buffer offset")?;
            let length = calls["length"].as_u64().ok_or("Invalid buffer length")?;
            let source = ptr(calls, "source_address")?;
            if calls["buffer_verified"] != true
                || offset.checked_add(length).is_none_or(|n| n > capacity)
                || source.checked_add(offset) != Some(ptr(&v, "x1")?)
                || ptr(&v, "x2")? != length
                || !mapped(source, capacity, b'r', None)
            {
                return Err("NewDirectByteBuffer address/capacity evidence did not pass".into());
            }
        }
        let rows = v["slots"]
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or("Expected exactly two captured JNI slots")?;
        let mut slots = BTreeMap::new();
        for row in rows {
            let slot = row["index"]
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or("Invalid JNI slot")?;
            let name = operation(slot).ok_or("Unsupported JNI slot")?;
            if text(row, "name")? != name {
                return Err("JNI slot/name mismatch".into());
            }
            let address = ptr(row, "value")?;
            let module = &row["module"];
            let path = text(module, "path")?.to_string();
            let base = ptr(module, "base")?;
            let offset = ptr(module, "offset")?;
            let code = bytes(text(row, "code16")?)?;
            if !path.ends_with("/libart.so")
                || base.checked_add(offset) != Some(address)
                || !address.is_multiple_of(4)
                || !mapped(address, 16, b'x', Some(&path))
                || !runtime.segments.iter().any(|s| {
                    s.permissions.is_some_and(|p| p & 4 != 0)
                        && s.read_bytes(offset, 16) == Some(code.as_slice())
                })
            {
                return Err("JNI target runtime map/ART code mismatch".into());
            }
            if slots
                .insert(
                    slot,
                    Target {
                        address,
                        offset,
                        path,
                    },
                )
                .is_some()
            {
                return Err("Duplicate JNI slot".into());
            }
        }
        let required = if terminal_slot == 14 {
            [6, 14]
        } else {
            [229, 230]
        };
        if !slots.keys().copied().eq(required) {
            return Err("Wrong JNI slot pair for capture schema".into());
        }
        if slots[&terminal_slot].address != ptr(&v, "x3")? {
            return Err("Observed BR x3 disagrees with captured JNI operation".into());
        }
        let mut expected = BTreeMap::new();
        for s in &image.segments {
            if s.permissions.is_some_and(|p| p & 4 != 0) {
                for (n, word) in s.data.chunks_exact(4).enumerate() {
                    expected.insert(
                        s.start_address + n as u64 * 4,
                        u32::from_le_bytes(word.try_into().unwrap()),
                    );
                }
            }
        }
        if expected.get(&branch) != Some(&0xd61f0060) {
            return Err("Captured BR is not present in original ELF".into());
        }
        Ok(Self {
            terminal_slot,
            entry,
            branch,
            env,
            table,
            slots,
            pid,
            fingerprint,
            expected,
        })
    }
    pub fn summary(&self) -> String {
        let mut out = format!("JNI runtime context: offline snapshot, PID {}, {}; entry=0x{:x}, env=0x{:x}, table=0x{:x}\nRuntime scope: supplied capture only; stable JNIEnv table assumed; external ART body not analyzed\n",self.pid,self.fingerprint,self.entry,self.env,self.table);
        for (&slot, target) in &self.slots {
            out.push_str(&format!(
                "JNI slot {slot}: {} -> 0x{:x} ({}+0x{:x})\n",
                operation(slot).unwrap(),
                target.address,
                target.path,
                target.offset
            ));
        }
        out
    }
    /// Must-dataflow over the existing decoded CFG. No target code is imported,
    /// and no instructions are decoded outside the original instruction budget.
    pub fn resolve(&self, prog: &mut GhidraProgram, proc: &mut Procedure) -> Result<usize, String> {
        if proc.entry_address != self.entry {
            return Err("JNI snapshot entry does not match requested analysis".into());
        }
        let decoded: BTreeMap<_, _> = proc
            .basic_blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|i| (i.address, i.clone()))
            .collect();
        if decoded.values().any(|i| {
            i.pcode
                .iter()
                .any(|op| matches!(op, PcodeOp::RuntimeJniCall { .. }))
        }) {
            return Err("JNI context already applied; rebuild analysis from the ELF".into());
        }
        for (&pc, i) in &decoded {
            let word = u32::from_le_bytes(i.bytes);
            if self.expected.get(&pc) != Some(&word)
                || prog.segment_at(pc).and_then(|s| s.read_u32_le(pc)) != Some(word)
            {
                return Err("JNI dataflow input no longer matches captured ELF".into());
            }
        }
        let mut initial = State {
            regs: [None; 32],
            valid: true,
        };
        initial.regs[0] = Some(Abstract::Env(0));
        initial.regs[31] = Some(Abstract::Stack(0));
        let mut states = BTreeMap::from([(self.entry, initial)]);
        let mut pending = BTreeSet::from([self.entry]);
        let mut work = 0usize;
        while let Some(pc) = pending.pop_first() {
            work += 1;
            if work > decoded.len().saturating_mul(128) {
                return Err("JNI dataflow did not converge".into());
            }
            let Some(i) = decoded.get(&pc) else {
                continue;
            };
            let state = transfer(i, states[&pc], self).0;
            for next in crate::arm64_jump_table::successors(i)
                .into_iter()
                .filter(|pc| decoded.contains_key(pc))
            {
                let merged = states.get(&next).map_or(state, |old| old.meet(state));
                if states.get(&next) != Some(&merged) {
                    states.insert(next, merged);
                    pending.insert(next);
                }
            }
        }
        let mut found = 0;
        for i in proc
            .basic_blocks
            .iter_mut()
            .flat_map(|b| &mut b.instructions)
        {
            let Some(state) = states.get(&i.address).copied() else {
                continue;
            };
            if let Some(slot) = transfer(i, state, self).1 {
                let tail = matches!(i.pcode.first(), Some(PcodeOp::BranchInd { .. }));
                let target_reg = match i.pcode.first().unwrap() {
                    PcodeOp::BranchInd { target_reg, .. } => *target_reg,
                    PcodeOp::CallInd { target } => *target,
                    _ => continue,
                };
                let target = &self.slots[&slot];
                i.pcode[0] = PcodeOp::RuntimeJniCall {
                    target_reg,
                    table: self.table,
                    target: target.address,
                    slot,
                    tail,
                };
                prog.add_memory_reference(
                    i.address,
                    target.address,
                    if tail {
                        GhidraRefType::ComputedJump
                    } else {
                        GhidraRefType::UnconditionalCall
                    },
                    GhidraSourceType::Analysis,
                    0,
                );
                prog.eol_comments.insert(
                    i.address,
                    format!(
                        "Captured JNI {}: {}+0x{:x}, runtime=0x{:x}; snapshot scope",
                        operation(slot).unwrap(),
                        target.path,
                        target.offset,
                        target.address
                    ),
                );
                found += 1;
            }
        }
        Ok(found)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Abstract {
    Env(u64),
    Table(u64),
    Stack(u64),
    Function(u16),
    Constant(u64),
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct State {
    regs: [Option<Abstract>; 32],
    valid: bool,
}
impl State {
    fn meet(&self, other: Self) -> Self {
        Self {
            regs: std::array::from_fn(|n| {
                if self.regs[n] == other.regs[n] {
                    self.regs[n]
                } else {
                    None
                }
            }),
            valid: self.valid && other.valid,
        }
    }
}
fn get(v: Varnode, s: &State, t: &BTreeMap<u64, Option<Abstract>>) -> Option<Abstract> {
    let value = match v.space {
        SpaceKind::Constant => Some(Abstract::Constant(v.offset)),
        SpaceKind::Register => s.regs[v.offset as usize],
        SpaceKind::Unique => t.get(&v.offset).copied().flatten(),
        _ => None,
    };
    if v.size == 8 {
        value
    } else {
        None
    }
}
fn put(
    v: Varnode,
    value: Option<Abstract>,
    s: &mut State,
    t: &mut BTreeMap<u64, Option<Abstract>>,
) {
    let value = if v.size == 8 { value } else { None };
    match v.space {
        SpaceKind::Register => s.regs[v.offset as usize] = value,
        SpaceKind::Unique => {
            t.insert(v.offset, value);
        }
        _ => {}
    }
}
fn add(a: Option<Abstract>, b: Option<Abstract>) -> Option<Abstract> {
    let (Some(a), Some(Abstract::Constant(b))) = (a, b) else {
        return None;
    };
    Some(match a {
        Abstract::Env(n) => Abstract::Env(n.wrapping_add(b)),
        Abstract::Table(n) => Abstract::Table(n.wrapping_add(b)),
        Abstract::Stack(n) => Abstract::Stack(n.wrapping_add(b)),
        Abstract::Constant(n) => Abstract::Constant(n.wrapping_add(b)),
        _ => return None,
    })
}
fn transfer(
    i: &DecodedInstruction,
    mut s: State,
    snapshot: &RuntimeSnapshot,
) -> (State, Option<u16>) {
    let mut t = BTreeMap::new();
    let mut resolved = None;
    for op in &i.pcode {
        let value = match op {
            PcodeOp::Copy { src, .. } => get(*src, &s, &t),
            PcodeOp::ZeroExtend { dst, src } if dst.size == src.size => get(*src, &s, &t),
            PcodeOp::IntAdd { lhs, rhs, .. } => add(get(*lhs, &s, &t), get(*rhs, &s, &t)),
            PcodeOp::Load { dst, addr } if dst.size == 8 && s.valid => match get(*addr, &s, &t) {
                Some(Abstract::Env(0)) => Some(Abstract::Table(0)),
                Some(Abstract::Table(n)) if n % 8 == 0 => u16::try_from(n / 8)
                    .ok()
                    .filter(|n| snapshot.slots.contains_key(n))
                    .map(Abstract::Function),
                _ => None,
            },
            _ => None,
        };
        match op {
            PcodeOp::Store { addr, .. }
            | PcodeOp::VectorStore { addr, .. }
            | PcodeOp::VectorStoreSized { addr, .. } => {
                if !matches!(get(*addr, &s, &t), Some(Abstract::Stack(_))) {
                    s.valid = false;
                }
            }
            PcodeOp::AtomicAdd { .. } | PcodeOp::ExclusiveStoreRelease { .. } => s.valid = false,
            PcodeOp::CallInd { target } => {
                if s.valid {
                    if let Some(Abstract::Function(slot)) = get(*target, &s, &t) {
                        if s.regs[0] == Some(Abstract::Env(0)) {
                            resolved = Some(slot);
                        }
                    }
                }
            }
            PcodeOp::BranchInd {
                target_reg,
                switch_targets,
            } if switch_targets.is_empty() => {
                if s.valid
                    && i.address == snapshot.branch
                    && s.regs[0] == Some(Abstract::Env(0))
                    && get(*target_reg, &s, &t) == Some(Abstract::Function(snapshot.terminal_slot))
                {
                    resolved = Some(snapshot.terminal_slot);
                }
            }
            _ => {}
        }
        if let Some(dst) = op.value_destination() {
            put(dst, value, &mut s, &mut t);
        }
    }
    if i.is_call {
        if resolved.is_none() {
            s.valid = false;
        }
        for n in 0..=18 {
            s.regs[n] = None;
        }
        s.regs[30] = None;
    }
    (s, resolved)
}
