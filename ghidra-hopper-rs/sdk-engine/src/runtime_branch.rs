//! Explicit controlled-process observations. External callback bindings are
//! conditional on the supplied entry argument; a null resolver remains INCOMPLETE.
use crate::{
    cfg_tags::Procedure,
    ghidra_port::{GhidraProgram, GhidraRefType, GhidraSourceType},
    jni_runtime::sha256,
    macho_objc::MachOImage,
    pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("Missing string {key}"))
}
fn ptr(v: &Value, key: &str) -> Result<u64, String> {
    u64::from_str_radix(
        text(v, key)?
            .strip_prefix("0x")
            .ok_or("Expected hex pointer")?,
        16,
    )
    .map_err(|_| format!("Invalid pointer {key}"))
}
fn mapped(maps: &[Value], address: u64, size: u64, execute: bool, path: &str) -> bool {
    maps.iter().filter_map(Value::as_str).any(|line| {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.len() < 6 || f[5] != path {
            return false;
        }
        let Some((lo, hi)) = f[0].split_once('-') else {
            return false;
        };
        let (Ok(lo), Ok(hi)) = (u64::from_str_radix(lo, 16), u64::from_str_radix(hi, 16)) else {
            return false;
        };
        lo <= address
            && address.checked_add(size).is_some_and(|n| n <= hi)
            && f[1].as_bytes().get(if execute { 2 } else { 0 })
                == Some(&if execute { b'x' } else { b'r' })
    })
}
#[derive(Debug, Clone)]
pub struct RuntimeBranchSnapshot {
    entry: u64,
    branch: u64,
    register: u64,
    target: u64,
    input: Option<u64>,
    memory: Option<u64>,
    pid: u64,
    expected: BTreeMap<u64, u32>,
    description: String,
}
impl RuntimeBranchSnapshot {
    pub fn parse(profile: &[u8], binary: &[u8], provider: &[u8]) -> Result<Self, String> {
        if profile.len() > 262144 || provider.len() > 64 * 1024 * 1024 {
            return Err("Runtime evidence size limit".into());
        }
        let v: Value = serde_json::from_slice(profile).map_err(|e| e.to_string())?;
        if v["schema"] != 1
            || text(&v, "source")? != "android-process-private-branch"
            || text(&v, "abi")? != "arm64-v8a"
        {
            return Err("Unsupported branch capture schema/source/ABI".into());
        }
        if text(&v, "binary_sha256")? != sha256(binary)
            || text(&v, "runtime_sha256")? != sha256(provider)
        {
            return Err("Branch evidence image SHA-256 mismatch".into());
        }
        if !binary.starts_with(b"\x7fELF") || !provider.starts_with(b"\x7fELF") {
            return Err("Branch evidence requires original ELF images".into());
        }
        let image = MachOImage::parse(binary)?;
        let runtime = MachOImage::parse(provider)?;
        if image.selected_arch != "arm64" || runtime.selected_arch != "arm64" {
            return Err("Branch images must be ARM64".into());
        }
        let pid = v["pid"]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or("Invalid capture PID")?;
        if text(&v, "fingerprint")?.is_empty() {
            return Err("Missing build fingerprint".into());
        }
        let module = &v["target_module"];
        let base = ptr(module, "base")?;
        let entry = ptr(module, "offset")?;
        if base.checked_add(entry) != Some(ptr(&v, "entry")?) {
            return Err("Branch entry/load bias mismatch".into());
        }
        let branch = ptr(&v, "branch_pc")?
            .checked_sub(base)
            .ok_or("Branch load bias mismatch")?;
        let register = v["target_register"]
            .as_u64()
            .filter(|n| *n < 31)
            .ok_or("Invalid BR register")?;
        let word = 0xd61f0000 | ((register as u32) << 5);
        if ptr(&v, "original_word")? != word as u64 || ptr(&v, "restored_word")? != word as u64 {
            return Err("Original/restored BR mismatch".into());
        }
        let maps = v["maps"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 64)
            .ok_or("Invalid branch maps")?;
        let path = text(module, "path")?;
        if !mapped(maps, ptr(&v, "entry")?, 4, true, path)
            || !mapped(maps, ptr(&v, "branch_pc")?, 4, true, path)
        {
            return Err("Original code not in executable runtime map".into());
        }
        let mut expected = BTreeMap::new();
        for s in &image.segments {
            if s.permissions.is_some_and(|p| p & 4 != 0) {
                for (n, bytes) in s.data.chunks_exact(4).enumerate() {
                    expected.insert(
                        s.start_address + n as u64 * 4,
                        u32::from_le_bytes(bytes.try_into().unwrap()),
                    );
                }
            }
        }
        if !expected.contains_key(&entry) || expected.get(&branch) != Some(&word) {
            return Err("Captured code is not present in original ELF".into());
        }
        let registers = v["registers"]
            .as_array()
            .filter(|a| a.len() == 31)
            .ok_or("Missing BR registers")?;
        let observed: Vec<u64> = registers
            .iter()
            .map(|s| {
                let value = s.as_str().ok_or("Invalid register")?;
                u64::from_str_radix(value.strip_prefix("0x").ok_or("Expected hex register")?, 16)
                    .map_err(|_| "Invalid register")
            })
            .collect::<Result<_, _>>()?;
        let target = ptr(&v, "observed_target")?;
        if observed[register as usize] != target || v["calls"]["trap_hits"] != 1 {
            return Err("BR observation does not match target".into());
        }
        let (input, memory, description) = match text(&v, "kind")? {
            "registered-callback" => {
                if register != 16
                    || target == 0
                    || !target.is_multiple_of(4)
                    || ptr(&v, "entry_x0")? != target
                    || observed[0] != target
                    || v["calls"]["uninstrumented"] != 1
                    || v["calls"]["instrumented"] != 1
                    || v["calls"]["null_guard"] != true
                    || v["calls"]["registered"] != true
                    || v["calls"]["finalized"] != true
                {
                    return Err("Registered callback execution evidence failed".into());
                }
                let t = &v["target"];
                let m = &t["module"];
                let offset = ptr(m, "offset")?;
                let code = text(t, "code16")?;
                if code.len() != 32 || !code.is_ascii() {
                    return Err("Expected 16 target bytes".into());
                }
                let bytes: Vec<u8> = (0..32)
                    .step_by(2)
                    .map(|n| {
                        u8::from_str_radix(&code[n..n + 2], 16).map_err(|_| "Invalid target code")
                    })
                    .collect::<Result<_, _>>()?;
                if ptr(m, "base")?.checked_add(offset) != Some(target)
                    || !mapped(maps, target, 16, true, text(m, "path")?)
                    || !runtime.segments.iter().any(|s| {
                        s.permissions.is_some_and(|p| p & 4 != 0)
                            && s.read_bytes(offset, 16) == Some(bytes.as_slice())
                    })
                {
                    return Err("Callback provider map/code mismatch".into());
                }
                (Some(target),None,format!("registered callback {}+0x{offset:x}; conditional on entry x0=0x{target:x}; signature unknown",text(m,"path")?))
            }
            "null-plt-resolver" => {
                let m = &v["memory"];
                let offset = ptr(m, "offset")?;
                let address = base.checked_add(offset).ok_or("GOT address overflow")?;
                if register != 17
                    || target != 0
                    || ptr(m, "before")? != 0
                    || ptr(m, "after")? != 0
                    || ptr(m, "address")? != address
                    || observed[16] != address
                    || !mapped(maps, address, 8, false, path)
                    || !image
                        .segments
                        .iter()
                        .any(|s| s.read_bytes(offset, 8).is_some())
                    || v["calls"]["plain_faults"] != 1
                    || v["calls"]["traced_faults"] != 1
                    || v["calls"]["signal"] != 11
                    || ptr(&v["calls"], "fault_pc")? != 0
                    || ptr(&v["calls"], "fault_address")? != 0
                {
                    return Err("Null resolver GOT/fault observation failed".into());
                }
                if sha256(provider) != sha256(binary) {
                    return Err("Null resolver requires same original ELF as provider".into());
                }
                (None,Some(offset),format!("GOT+0x{offset:x}=0; plain and traced SIGSEGV at PC=0; not a callable target; INCOMPLETE retained"))
            }
            _ => return Err("Unsupported branch capture kind".into()),
        };
        Ok(Self {
            entry,
            branch,
            register,
            target,
            input,
            memory,
            pid,
            expected,
            description,
        })
    }
    pub fn summary(&self) -> String {
        format!("Runtime branch context: supplied offline snapshot, PID {}, entry=0x{:x}, branch=0x{:x}\nRuntime scope: controlled replay only; {}\n",self.pid,self.entry,self.branch,self.description)
    }
    pub fn resolve(&self, prog: &mut GhidraProgram, proc: &mut Procedure) -> Result<usize, String> {
        if proc.entry_address != self.entry {
            return Err("Branch snapshot entry does not match analysis".into());
        }
        let decoded: BTreeMap<_, _> = proc
            .basic_blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|i| (i.address, i.clone()))
            .collect();
        for (&pc, i) in &decoded {
            if self.expected.get(&pc) != Some(&u32::from_le_bytes(i.bytes))
                || prog.segment_at(pc).and_then(|s| s.read_u32_le(pc))
                    != Some(u32::from_le_bytes(i.bytes))
            {
                return Err("Branch analysis code changed from captured ELF".into());
            }
            if i.pcode.iter().any(|op| {
                matches!(
                    op,
                    PcodeOp::RuntimeTailCall { .. } | PcodeOp::RuntimeInvalidBranch { .. }
                )
            }) {
                return Err("Branch context already applied; rebuild from ELF".into());
            }
        }
        let mut initial = State {
            regs: [None; 32],
            valid: true,
        };
        initial.regs[0] = self.input.map(Origin::Entry);
        initial.regs[31] = Some(Origin::Stack(0));
        let mut states = BTreeMap::from([(self.entry, initial)]);
        let mut pending = BTreeSet::from([self.entry]);
        let mut work = 0usize;
        while let Some(pc) = pending.pop_first() {
            work += 1;
            if work > decoded.len().saturating_mul(128) {
                return Err("Branch dataflow did not converge".into());
            }
            let Some(i) = decoded.get(&pc) else {
                continue;
            };
            let state = transfer(i, states[&pc], self);
            for next in crate::arm64_jump_table::successors(i)
                .into_iter()
                .filter(|n| decoded.contains_key(n))
            {
                let merged = states.get(&next).map_or(state, |old| old.meet(state));
                if states.get(&next) != Some(&merged) {
                    states.insert(next, merged);
                    pending.insert(next);
                }
            }
        }
        let Some(state) = states.get(&self.branch) else {
            return Ok(0);
        };
        let proven = if self.memory.is_some() {
            Origin::Memory(self.target)
        } else {
            Origin::Entry(self.target)
        };
        if !state.valid || state.regs[self.register as usize] != Some(proven) {
            return Ok(0);
        }
        let i = proc
            .basic_blocks
            .iter_mut()
            .flat_map(|b| &mut b.instructions)
            .find(|i| i.address == self.branch)
            .ok_or("Missing captured branch")?;
        let Some(PcodeOp::BranchInd {
            target_reg,
            switch_targets,
        }) = i.pcode.first()
        else {
            return Ok(0);
        };
        if !switch_targets.is_empty() || target_reg.offset != self.register || target_reg.size != 8
        {
            return Ok(0);
        }
        let target_reg = *target_reg;
        i.pcode[0] = if self.memory.is_some() {
            PcodeOp::RuntimeInvalidBranch {
                target_reg,
                target: self.target,
                signal: 11,
            }
        } else {
            prog.add_memory_reference(
                self.branch,
                self.target,
                GhidraRefType::ComputedJump,
                GhidraSourceType::Analysis,
                0,
            );
            PcodeOp::RuntimeTailCall {
                target_reg,
                target: self.target,
            }
        };
        prog.eol_comments
            .insert(self.branch, self.description.clone());
        Ok(1)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Value(u64),
    Entry(u64),
    Memory(u64),
    Stack(u64),
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct State {
    regs: [Option<Origin>; 32],
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
fn get(v: Varnode, s: &State, t: &BTreeMap<u64, Option<Origin>>) -> Option<Origin> {
    if v.size != 8 {
        return None;
    }
    match v.space {
        SpaceKind::Constant => Some(Origin::Value(v.offset)),
        SpaceKind::Register => s.regs[v.offset as usize],
        SpaceKind::Unique => t.get(&v.offset).copied().flatten(),
        _ => None,
    }
}
fn transfer(i: &DecodedInstruction, mut s: State, snapshot: &RuntimeBranchSnapshot) -> State {
    let mut t = BTreeMap::new();
    for op in &i.pcode {
        let value = match op {
            PcodeOp::Copy { src, .. } => get(*src, &s, &t),
            PcodeOp::ZeroExtend { dst, src } if dst.size == src.size => get(*src, &s, &t),
            PcodeOp::IntAdd { lhs, rhs, .. } => match (get(*lhs, &s, &t), get(*rhs, &s, &t)) {
                (Some(Origin::Value(a)), Some(Origin::Value(b))) => {
                    Some(Origin::Value(a.wrapping_add(b)))
                }
                (Some(Origin::Stack(a)), Some(Origin::Value(b))) => {
                    Some(Origin::Stack(a.wrapping_add(b)))
                }
                _ => None,
            },
            PcodeOp::Load { dst, addr } if dst.size == 8 && s.valid => match get(*addr, &s, &t) {
                Some(Origin::Value(n)) if Some(n) == snapshot.memory => {
                    Some(Origin::Memory(snapshot.target))
                }
                _ => None,
            },
            _ => None,
        };
        match op {
            PcodeOp::Store { addr, .. }
            | PcodeOp::VectorStore { addr, .. }
            | PcodeOp::VectorStoreSized { addr, .. } => {
                if !matches!(get(*addr, &s, &t), Some(Origin::Stack(_))) {
                    s.valid = false;
                }
            }
            PcodeOp::AtomicAdd { .. } | PcodeOp::ExclusiveStoreRelease { .. } => s.valid = false,
            _ => {}
        }
        if let Some(dst) = op.value_destination() {
            let value = if dst.size == 8 { value } else { None };
            match dst.space {
                SpaceKind::Register => s.regs[dst.offset as usize] = value,
                SpaceKind::Unique => {
                    t.insert(dst.offset, value);
                }
                _ => {}
            }
        }
    }
    if i.is_call {
        s.valid = false;
        for n in 0..=18 {
            s.regs[n] = None;
        }
        s.regs[30] = None;
    }
    s
}
