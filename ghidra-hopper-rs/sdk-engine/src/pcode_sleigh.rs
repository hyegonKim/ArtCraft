//! Ghidra SLEIGH / P-Code Intermediate Representation & Decompiler fused with Hopper's
//! Objective-C `CALL_OBJC` (`[receiver selector:arg]`) and Switch-Table Hints.
//!
//! By implementing Varnodes, PcodeOps, and ARM64 decoding natively in Rust:
//! 1. Zero JVM garbage-collection pauses or object-header bloat (`Varnode` is `Copy`, 24 bytes).
//! 2. Zero IPC pipe serialization overhead between the disassembler and the decompiler.

use crate::segment::{CallReference, CallType, OperandFormat, Segment};
use std::collections::BTreeMap;

/// Ghidra Varnode address space kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceKind {
    Register,
    Constant,
    Unique,
    Ram,
    Stack,
}

/// Compact, `Copy` Rust representation of a Ghidra `Varnode` (24 bytes, zero heap allocation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Varnode {
    pub space: SpaceKind,
    pub offset: u64,
    pub size: u8,
}

impl Varnode {
    pub const fn reg(idx: u64, size: u8) -> Self {
        Self {
            space: SpaceKind::Register,
            offset: idx,
            size,
        }
    }

    pub const fn imm(val: u64, size: u8) -> Self {
        Self {
            space: SpaceKind::Constant,
            offset: val,
            size,
        }
    }

    pub const fn ram(addr: u64, size: u8) -> Self {
        Self {
            space: SpaceKind::Ram,
            offset: addr,
            size,
        }
    }

    pub const fn stack(disp: i64, size: u8) -> Self {
        Self {
            space: SpaceKind::Stack,
            offset: disp as u64,
            size,
        }
    }

    pub fn display_arm64(&self) -> String {
        match self.space {
            SpaceKind::Register => match self.offset {
                31 => if self.size == 4 { "wsp" } else { "sp" }.to_string(),
                30 if self.size == 8 => "lr".to_string(),
                29 if self.size == 8 => "fp".to_string(),
                idx if self.size == 4 => format!("w{}", idx),
                idx => format!("x{}", idx),
            },
            SpaceKind::Constant => format!("0x{:x}", self.offset),
            SpaceKind::Ram => format!("*[ram]0x{:x}", self.offset),
            SpaceKind::Stack => {
                let signed = self.offset as i64;
                if signed < 0 {
                    format!("var_{:x}", (-signed) as u64)
                } else {
                    format!("arg_{:x}", signed as u64)
                }
            }
            SpaceKind::Unique => format!("u_{:x}", self.offset),
        }
    }

    pub fn display_arm64_with_aliases(&self, aliases: &BTreeMap<u64, String>) -> String {
        if self.space == SpaceKind::Register {
            if let Some(custom) = aliases.get(&self.offset) {
                return custom.clone();
            }
        }
        self.display_arm64()
    }
}

/// Core subset of Ghidra P-Code opcodes (`OpCode`), augmented with `CallObjC` for Hopper parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitfieldKind {
    Unsigned,
    Signed,
    Insert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcodeOp {
    ConditionalCompare {
        lhs: Varnode,
        rhs: Varnode,
        width: u8,
        condition: crate::arm64_flags::Condition,
        nzcv: u8,
        subtract: bool,
    },
    SignedMultiplyHigh {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
    },
    SignedMultiplyAddLong {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
        addend: Varnode,
    },
    MultiplyAdd {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
        addend: Varnode,
        width: u8,
        subtract: bool,
    },
    Logical {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
        op: crate::arm64_integer::Logic,
        invert: bool,
        shift: crate::arm64_logical::LogicalShift,
        amount: u8,
        width: u8,
        set_flags: bool,
    },
    ConditionalTransform {
        dst: Option<Varnode>,
        if_true: Varnode,
        if_false: Varnode,
        condition: crate::arm64_flags::Condition,
        action: crate::arm64_integer::SelectAction,
        width: u8,
    },
    RegisterArithmetic {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
        extension: Option<u8>,
        shift: crate::arm64_flags::Shift,
        amount: u8,
        width: u8,
        subtract: bool,
        set_flags: bool,
    },
    VectorImmediate {
        register: u8,
        low: u64,
        high: u64,
    },
    VectorLoad {
        register: u8,
        addr: Varnode,
    },
    VectorStore {
        register: u8,
        addr: Varnode,
    },
    VectorLoadSized {
        register: u8,
        addr: Varnode,
        size: u8,
    },
    VectorStoreSized {
        register: u8,
        addr: Varnode,
        size: u8,
    },
    AtomicAdd {
        dst: Varnode,
        src: Varnode,
        addr: Varnode,
        acquire: bool,
        release: bool,
    },
    ExclusiveLoadAcquire {
        dst: Varnode,
        addr: Varnode,
    },
    ExclusiveStoreRelease {
        status: Varnode,
        src: Varnode,
        addr: Varnode,
    },
    ClearExclusive,
    ClobberExclusive,
    /// Base AAPCS64: V0-V7/V16-V31 and upper halves of V8-V15 are unknown.
    ClobberSimd,
    UnverifiedContinuation {
        target: u64,
    },
    MoveKeep {
        dst: Varnode,
        old: Varnode,
        immediate: u16,
        shift: u8,
    },
    ConditionalSelect {
        dst: Varnode,
        if_true: Varnode,
        if_false: Varnode,
        condition: crate::arm64_flags::Condition,
    },
    SignExtend {
        dst: Varnode,
        src: Varnode,
    },
    LoadAcquire {
        dst: Varnode,
        addr: Varnode,
    },
    ReadThreadPointer {
        dst: Varnode,
    },
    NoReturnCall {
        target: u64,
        symbol: String,
    },
    /// Bounded unsigned-halfword relative table, with the original index mapping.
    TableBranch {
        target_reg: Varnode,
        index: Varnode,
        table_address: u64,
        offset_base: u64,
        targets: Vec<u64>,
    },
    ShiftedAdd {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
        shift: crate::arm64_flags::Shift,
        amount: u8,
    },
    RegisterOffset {
        dst: Varnode,
        base: Varnode,
        index: Varnode,
        extend: crate::arm64_indexed::IndexExtend,
        shift: u8,
    },
    TestBitBranch {
        value: Varnode,
        bit: u8,
        nonzero: bool,
        target: u64,
    },
    IntOr {
        dst: Varnode,
        lhs: Varnode,
        rhs: Varnode,
        shift: crate::arm64_logical::LogicalShift,
        amount: u8,
    },
    FlagArithmetic {
        dst: Option<Varnode>,
        lhs: Varnode,
        rhs: Varnode,
        shift: crate::arm64_flags::Shift,
        amount: u8,
        subtract: bool,
    },
    FlagBranch {
        condition: crate::arm64_flags::Condition,
        target: u64,
    },
    ClobberNzcv,
    Bitfield {
        dst: Varnode,
        src: Varnode,
        old_dst: Varnode,
        kind: BitfieldKind,
        immr: u8,
        imms: u8,
    },
    ExternalTailCall {
        symbol: String,
        got_address: u64,
    },
    MachOImportTailCall {
        symbol: String,
        library: String,
        pointer_address: u64,
        selector_address: Option<u64>,
        selector: Option<String>,
    },
    Bti {
        kind: u8,
    },
    ZeroExtend {
        dst: Varnode,
        src: Varnode,
    },
    CallInd {
        target: Varnode,
    },
    /// External callback under an explicitly captured entry-argument context.
    RuntimeTailCall {
        target_reg: Varnode,
        target: u64,
    },
    /// Observed non-callable target; never makes a procedure complete.
    RuntimeInvalidBranch {
        target_reg: Varnode,
        target: u64,
        signal: u8,
    },
    /// JNI binding from explicit offline runtime evidence, scoped to that capture.
    RuntimeJniCall {
        target_reg: Varnode,
        table: u64,
        target: u64,
        slot: u16,
        tail: bool,
    },
    Unsupported {
        word: u32,
    },
    Copy {
        dst: Varnode,
        src: Varnode,
    },
    IntAdd {
        dst: Varnode,
        lhs: Varnode,
        rhs: Varnode,
    },
    IntSub {
        dst: Varnode,
        lhs: Varnode,
        rhs: Varnode,
    },
    IntAnd {
        dst: Varnode,
        lhs: Varnode,
        rhs: Varnode,
    },
    IntEqual {
        dst: Varnode,
        lhs: Varnode,
        rhs: Varnode,
    },
    Load {
        dst: Varnode,
        addr: Varnode,
    },
    Store {
        addr: Varnode,
        src: Varnode,
    },
    Branch {
        target: u64,
    },
    CBranch {
        cond: Varnode,
        target: u64,
        branch_on_zero: bool,
    },
    BranchInd {
        target_reg: Varnode,
        switch_targets: Vec<u64>,
    },
    Call {
        target: u64,
        symbol: Option<String>,
    },
    CallObjC {
        stub_addr: u64,
        selector: String,
    },
    Return {
        retval: Varnode,
    },
    Nop,
}

impl PcodeOp {
    /// Value output, including unique temporaries; NZCV/ABI effects are separate.
    pub fn value_destination(&self) -> Option<Varnode> {
        match self {
            Self::Copy { dst, .. }
            | Self::AtomicAdd { dst, .. }
            | Self::ExclusiveLoadAcquire { dst, .. }
            | Self::IntAdd { dst, .. }
            | Self::IntSub { dst, .. }
            | Self::IntAnd { dst, .. }
            | Self::IntOr { dst, .. }
            | Self::IntEqual { dst, .. }
            | Self::ZeroExtend { dst, .. }
            | Self::Load { dst, .. }
            | Self::Bitfield { dst, .. }
            | Self::RegisterOffset { dst, .. }
            | Self::MoveKeep { dst, .. }
            | Self::ConditionalSelect { dst, .. }
            | Self::SignExtend { dst, .. }
            | Self::LoadAcquire { dst, .. }
            | Self::ReadThreadPointer { dst, .. } => Some(*dst),
            Self::ExclusiveStoreRelease { status, .. } => Some(*status),
            Self::ShiftedAdd { dst, .. }
            | Self::FlagArithmetic { dst, .. }
            | Self::Logical { dst, .. }
            | Self::ConditionalTransform { dst, .. }
            | Self::RegisterArithmetic { dst, .. } => *dst,
            Self::MultiplyAdd { dst, .. }
            | Self::SignedMultiplyHigh { dst, .. }
            | Self::SignedMultiplyAddLong { dst, .. } => *dst,
            _ => None,
        }
    }
}

/// Decoded native instruction + lifted Ghidra P-Code micro-operations.
#[derive(Debug, Clone)]
pub struct DecodedInstruction {
    pub address: u64,
    pub bytes: [u8; 4],
    pub mnemonic: String,
    pub raw_args: Vec<String>,
    pub formatted_args: Vec<String>,
    pub is_conditional_jump: bool,
    pub is_unconditional_jump: bool,
    pub is_call: bool,
    pub is_ret: bool,
    pub branch_target: Option<u64>,
    pub call_ref: Option<CallReference>,
    pub pcode: Vec<PcodeOp>,
}

/// Hopper `SwitchHintWindowController` equivalent: user- or analyzer-supplied jump table targets.
#[derive(Debug, Clone, Default)]
pub struct SwitchHintTable {
    pub hints: BTreeMap<u64, Vec<u64>>,
}

/// Decodes an ARM64 32-bit little-endian instruction word and lifts it into Ghidra P-Code.
pub fn decode_and_lift_arm64(
    addr: u64,
    word: u32,
    seg: &Segment,
    objc_stubs: &BTreeMap<u64, String>,
    switch_hints: &SwitchHintTable,
) -> DecodedInstruction {
    let mut insn = decode_arm64_inner(addr, word, seg, objc_stubs, switch_hints);
    if insn.is_call {
        insn.pcode.push(PcodeOp::ClobberSimd);
        insn.pcode.push(PcodeOp::ClobberExclusive);
        insn.pcode.push(PcodeOp::ClobberNzcv);
    }
    insn
}

fn decode_arm64_inner(
    addr: u64,
    word: u32,
    seg: &Segment,
    objc_stubs: &BTreeMap<u64, String>,
    switch_hints: &SwitchHintTable,
) -> DecodedInstruction {
    if let Some(insn) = crate::arm64_integer::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_simd::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_atomic::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_data::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_address::decode(addr, word, seg) {
        return insn;
    }
    if let Some(insn) = crate::arm64_flags::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_test_branch::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_bitfield::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_memory::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_indexed::decode(addr, word) {
        return insn;
    }
    if let Some(insn) = crate::arm64_logical::decode(addr, word) {
        return insn;
    }
    let bytes = word.to_le_bytes();
    let fmt_arg = |op_idx: usize, val: u64| -> String {
        seg.operand_formats
            .get(&(addr, op_idx))
            .copied()
            .unwrap_or_default()
            .format_imm(val)
    };

    // 1. RET (0xd65f03c0)
    if word == 0xd65f03c0 {
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "ret".into(),
            raw_args: vec![],
            formatted_args: vec![],
            is_conditional_jump: false,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: true,
            branch_target: None,
            call_ref: None,
            pcode: vec![PcodeOp::Return {
                retval: Varnode::reg(0, 8),
            }],
        };
    }

    // 2.NOP (0xd503201f)
    if word == 0xd503201f {
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "nop".into(),
            raw_args: vec![],
            formatted_args: vec![],
            is_conditional_jump: false,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: false,
            branch_target: None,
            call_ref: None,
            pcode: vec![PcodeOp::Nop],
        };
    }

    // 3. BL <imm26> (0x94000000 .. 0x97ffffff) - Direct Call or ObjC Stub Call
    if (word & 0xfc000000) == 0x94000000 {
        let imm26 = (word & 0x03ff_ffff) as i32;
        let signed_imm = ((imm26 << 6) >> 6) as i64 * 4;
        let target = (addr as i64).wrapping_add(signed_imm) as u64;

        if let Some(selector) = objc_stubs.get(&target) {
            let arg = format!("objc_msgSend${}", selector);
            return DecodedInstruction {
                address: addr,
                bytes,
                mnemonic: "bl".into(),
                raw_args: vec![format!("0x{:x}", target)],
                formatted_args: vec![arg],
                is_conditional_jump: false,
                is_unconditional_jump: false,
                is_call: true,
                is_ret: false,
                branch_target: Some(target),
                call_ref: Some(CallReference {
                    call_type: CallType::ObjC,
                    from_address: addr,
                    to_address: target,
                    selector_hint: Some(selector.clone()),
                }),
                pcode: vec![PcodeOp::CallObjC {
                    stub_addr: target,
                    selector: selector.clone(),
                }],
            };
        }

        let sym = seg
            .get_demangled_name_at_address(target)
            .map(str::to_string);
        let formatted = sym.clone().unwrap_or_else(|| fmt_arg(0, target));
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "bl".into(),
            raw_args: vec![format!("0x{:x}", target)],
            formatted_args: vec![formatted],
            is_conditional_jump: false,
            is_unconditional_jump: false,
            is_call: true,
            is_ret: false,
            branch_target: Some(target),
            call_ref: Some(CallReference {
                call_type: CallType::Direct,
                from_address: addr,
                to_address: target,
                selector_hint: None,
            }),
            pcode: vec![PcodeOp::Call {
                target,
                symbol: sym,
            }],
        };
    }

    // 4. B <imm26> (0x14000000 .. 0x17ffffff) - Unconditional Branch
    if (word & 0xfc000000) == 0x14000000 {
        let imm26 = (word & 0x03ff_ffff) as i32;
        let signed_imm = ((imm26 << 6) >> 6) as i64 * 4;
        let target = (addr as i64).wrapping_add(signed_imm) as u64;
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "b".into(),
            raw_args: vec![format!("0x{:x}", target)],
            formatted_args: vec![fmt_arg(0, target)],
            is_conditional_jump: false,
            is_unconditional_jump: true,
            is_call: false,
            is_ret: false,
            branch_target: Some(target),
            call_ref: None,
            pcode: vec![PcodeOp::Branch { target }],
        };
    }

    // 5. CBZ / CBNZ Xt, <imm19>
    if (word & 0x7e000000) == 0x34000000 {
        let is_64 = (word >> 31) != 0;
        let is_nz = ((word >> 24) & 1) != 0;
        let rt = (word & 0x1f) as u64;
        let imm19 = ((word >> 5) & 0x7ffff) as i32;
        let signed_imm = ((imm19 << 13) >> 13) as i64 * 4;
        let target = (addr as i64).wrapping_add(signed_imm) as u64;
        let reg_vn = if rt == 31 {
            Varnode::imm(0, if is_64 { 8 } else { 4 })
        } else {
            Varnode::reg(rt, if is_64 { 8 } else { 4 })
        };
        let mnem = if is_nz { "cbnz" } else { "cbz" };
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: mnem.into(),
            raw_args: vec![reg_vn.display_arm64(), format!("0x{:x}", target)],
            formatted_args: vec![reg_vn.display_arm64(), fmt_arg(1, target)],
            is_conditional_jump: true,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: false,
            branch_target: Some(target),
            call_ref: None,
            pcode: vec![PcodeOp::CBranch {
                cond: reg_vn,
                target,
                branch_on_zero: !is_nz,
            }],
        };
    }

    // 7. BR Xn (0xd61f0000) - Indirect jump / Switch table dispatch
    if (word & 0xfffffc1f) == 0xd61f0000 {
        let rn = ((word >> 5) & 0x1f) as u64;
        let reg_vn = Varnode::reg(rn, 8);
        let targets = switch_hints.hints.get(&addr).cloned().unwrap_or_default();
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "br".into(),
            raw_args: vec![reg_vn.display_arm64()],
            formatted_args: vec![reg_vn.display_arm64()],
            is_conditional_jump: false,
            is_unconditional_jump: true,
            is_call: false,
            is_ret: false,
            branch_target: targets.first().copied(),
            call_ref: None,
            pcode: vec![PcodeOp::BranchInd {
                target_reg: reg_vn,
                switch_targets: targets,
            }],
        };
    }

    // 8. MOVZ Xd/Wd, #<imm16>, LSL #<shift>
    if (word & 0x7f800000) == 0x52800000
        && ((word >> 31) != 0 || ((word >> 21) & 3) < 2)
        && word & 31 != 31
    {
        let is_64 = (word >> 31) != 0;
        let rd = (word & 0x1f) as u64;
        let imm16 = ((word >> 5) & 0xffff) as u64;
        let hw = ((word >> 21) & 0x3) * 16;
        let val = imm16 << hw;
        let dst = Varnode::reg(rd, if is_64 { 8 } else { 4 });
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "mov".into(),
            raw_args: vec![dst.display_arm64(), format!("#0x{:x}", val)],
            formatted_args: vec![dst.display_arm64(), fmt_arg(1, val)],
            is_conditional_jump: false,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: false,
            branch_target: None,
            call_ref: None,
            pcode: vec![PcodeOp::Copy {
                dst,
                src: Varnode::imm(val, if is_64 { 8 } else { 4 }),
            }],
        };
    }

    // 9. ADD / SUB (immediate) Xd, Xn, #<imm12>
    if (word & 0x1f800000) == 0x11000000 && word & (1 << 29) == 0 {
        let is_64 = (word >> 31) != 0;
        let is_sub = ((word >> 30) & 1) != 0;
        let rd = (word & 0x1f) as u64;
        let rn = ((word >> 5) & 0x1f) as u64;
        let imm12 = ((word >> 10) & 0xfff) as u64;
        let shift = if ((word >> 22) & 1) != 0 { 12 } else { 0 };
        let val = imm12 << shift;
        let sz = if is_64 { 8 } else { 4 };
        let dst = Varnode::reg(rd, sz);
        let lhs = Varnode::reg(rn, sz);
        let rhs = Varnode::imm(val, sz);
        let mnem = if is_sub { "sub" } else { "add" };
        let pcode_op = if is_sub {
            PcodeOp::IntSub { dst, lhs, rhs }
        } else {
            PcodeOp::IntAdd { dst, lhs, rhs }
        };
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: mnem.into(),
            raw_args: vec![
                dst.display_arm64(),
                lhs.display_arm64(),
                format!("#0x{:x}", val),
            ],
            formatted_args: vec![dst.display_arm64(), lhs.display_arm64(), fmt_arg(2, val)],
            is_conditional_jump: false,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: false,
            branch_target: None,
            call_ref: None,
            pcode: vec![pcode_op],
        };
    }

    // Fallback generic instruction representation
    DecodedInstruction {
        address: addr,
        bytes,
        mnemonic: "inst.arm64".into(),
        raw_args: vec![format!("0x{:08x}", word)],
        formatted_args: vec![OperandFormat::default().format_imm(word as u64)],
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![PcodeOp::Unsupported { word }],
    }
}

/// Hybrid Ghidra P-Code + Hopper Objective-C Structured Decompiler.
/// Folds register constants/ADRP+ADD pairs, applies Hopper register aliases (`self`, `_cmd`, user renames),
/// and emits clean C / Objective-C pseudocode.
pub fn decompile_instructions_to_pseudocode(
    proc_name: &str,
    entry_addr: u64,
    insns: &[DecodedInstruction],
    seg: &Segment,
) -> String {
    decompile_instructions_to_pseudocode_with_aliases(
        proc_name,
        entry_addr,
        insns,
        seg,
        &BTreeMap::new(),
    )
}

pub fn decompile_instructions_to_pseudocode_with_aliases(
    proc_name: &str,
    entry_addr: u64,
    insns: &[DecodedInstruction],
    seg: &Segment,
    reg_aliases: &BTreeMap<u64, String>,
) -> String {
    let mut out = String::new();
    let is_objc_method = proc_name.starts_with("-[") || proc_name.starts_with("+[");
    let sig_params = if is_objc_method {
        "id self, SEL _cmd"
    } else {
        ""
    };
    out.push_str(&format!(
        "// Experimental partial ARM64 pseudocode; no SSA or type recovery\nint64_t {}({}) {{ // @ 0x{:x}\n",
        proc_name, sig_params, entry_addr
    ));

    let mut reg_consts: BTreeMap<u64, u64> = BTreeMap::new();
    out.push_str("    nzcv = unknown_nzcv_at_entry(); // incoming flags are not assumed\n");
    out.push_str(
        "    exclusive_monitor = unknown_monitor_at_entry(); // reservation is not assumed\n",
    );
    let targets: std::collections::BTreeSet<_> = insns
        .iter()
        .filter(|insn| !insn.is_call)
        .filter_map(|insn| insn.branch_target)
        .collect();

    for insn in insns {
        if let Some(PcodeOp::Unsupported { word }) = insn.pcode.first() {
            out.push_str(&format!("    /* INCOMPLETE: unsupported instruction 0x{word:08x} at 0x{:x}; analysis stopped */\n", insn.address));
            break;
        }
        // Sorted instructions are not a dataflow merge. Clear at control-flow joins.
        if targets.contains(&insn.address) {
            reg_consts.clear();
        }
        out.push_str(&format!("loc_{:x}:\n", insn.address));
        for op in &insn.pcode {
            if !matches!(op, PcodeOp::Copy { .. } | PcodeOp::IntAdd { .. }) {
                if let Some(dst) = op
                    .value_destination()
                    .filter(|d| d.space == SpaceKind::Register)
                {
                    reg_consts.remove(&dst.offset);
                }
            }
            match op {
                PcodeOp::ConditionalCompare {lhs,rhs,width,condition,nzcv,subtract} => out.push_str(&format!("    nzcv = conditional_compare_u{width}(nzcv, {}, {}, {}, fallback=0x{nzcv:x}, subtract={subtract}); // condition reads incoming NZCV; no GPR write\n",condition.expression(),lhs.display_arm64_with_aliases(reg_aliases),rhs.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::SignedMultiplyHigh {dst,lhs,rhs} => out.push_str(&format!("    {}signed_multiply_high_u64({}, {}); // signed 128-bit product bits 127:64; NZCV preserved\n",dst.map(|v|format!("{} = ",v.display_arm64_with_aliases(reg_aliases))).unwrap_or_default(),lhs.display_arm64_with_aliases(reg_aliases),rhs.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::SignedMultiplyAddLong {dst,lhs,rhs,addend} => out.push_str(&format!("    {}signed_multiply_add_long_u64({}, {}, {}); // signed 32-bit sources, wrapping 64-bit sum; NZCV preserved\n",dst.map(|v|format!("{} = ",v.display_arm64_with_aliases(reg_aliases))).unwrap_or_default(),lhs.display_arm64_with_aliases(reg_aliases),rhs.display_arm64_with_aliases(reg_aliases),addend.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::MultiplyAdd {dst,lhs,rhs,addend,width,subtract} => out.push_str(&format!("    {}multiply_add_u{width}({}, {}, {}, subtract={subtract}); // wrapping integer arithmetic; NZCV preserved\n",dst.map(|v|format!("{} = ",v.display_arm64_with_aliases(reg_aliases))).unwrap_or_default(),lhs.display_arm64_with_aliases(reg_aliases),rhs.display_arm64_with_aliases(reg_aliases),addend.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::Logical { dst, lhs, rhs, op, invert, shift, amount, width, set_flags } => {
                    let result = format!("logical_{op:?}_u{width}({}, {}, invert={invert}, shift={shift:?}, amount={amount})", lhs.display_arm64_with_aliases(reg_aliases), rhs.display_arm64_with_aliases(reg_aliases));
                    out.push_str(&format!("    logical_result = {result};\n"));
                    if let Some(dst)=dst { out.push_str(&format!("    {} = logical_result;\n",dst.display_arm64_with_aliases(reg_aliases))); }
                    if *set_flags { out.push_str(&format!("    nzcv = logical_nzcv_u{width}(logical_result); // N/Z from result, C=0, V=0\n")); }
                },
                PcodeOp::ConditionalTransform { dst, if_true, if_false, condition, action, width } => {
                    out.push_str(&format!("    {}select_transform_u{width}({}, {}, {}, false_action={action:?}); // NZCV preserved\n",dst.map(|v|format!("{} = ",v.display_arm64_with_aliases(reg_aliases))).unwrap_or_default(),condition.expression(),if_true.display_arm64_with_aliases(reg_aliases),if_false.display_arm64_with_aliases(reg_aliases)));
                },
                PcodeOp::RegisterArithmetic { dst, lhs, rhs, extension, shift, amount, width, subtract, set_flags } => {
                    out.push_str(&format!("    (integer_result, integer_flags) = register_arithmetic_u{width}({}, {}, extension={extension:?}, shift={shift:?}, amount={amount}, subtract={subtract});\n",lhs.display_arm64_with_aliases(reg_aliases),rhs.display_arm64_with_aliases(reg_aliases)));
                    if let Some(dst)=dst {out.push_str(&format!("    {} = integer_result;\n",dst.display_arm64_with_aliases(reg_aliases)));}
                    if *set_flags {out.push_str("    nzcv = integer_flags;\n");}
                },
                PcodeOp::VectorImmediate{register,low,high} => out.push_str(&format!("    v{register} = u128_from_halves(0x{low:016x}, 0x{high:016x}); // separate SIMD bank; NZCV preserved\n")),
                PcodeOp::VectorLoad{register,addr} => out.push_str(&format!("    v{register} = load_u128({});\n",addr.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::VectorStore{register,addr} => out.push_str(&format!("    store_u128({}, v{register});\n",addr.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::VectorLoadSized{register,addr,size} => out.push_str(&format!("    v{register} = zero_extend_to_u128(load_u{}({})); // raw bits; clears upper SIMD bits\n",size*8,addr.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::VectorStoreSized{register,addr,size} => out.push_str(&format!("    store_u{}({}, low_bits(v{register}, {}));\n",size*8,addr.display_arm64_with_aliases(reg_aliases),size*8)),
                PcodeOp::AtomicAdd{dst,src,addr,acquire,release} => out.push_str(&format!("    {} = atomic_fetch_add_u{}({}, {}, acquire={}, release={}); // inputs read before output; NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases),dst.size*8,addr.display_arm64_with_aliases(reg_aliases),src.display_arm64_with_aliases(reg_aliases),acquire,release)),
                PcodeOp::ExclusiveLoadAcquire{dst,addr} => out.push_str(&format!("    {} = exclusive_load_acquire_u{}({}, &exclusive_monitor); // establishes reservation; NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases),dst.size*8,addr.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::ExclusiveStoreRelease{status,src,addr} => out.push_str(&format!("    {} = exclusive_store_release_u{}({}, {}, &exclusive_monitor); // 0=stored, 1=failed; consumes reservation; spurious failure allowed\n",status.display_arm64_with_aliases(reg_aliases),src.size*8,addr.display_arm64_with_aliases(reg_aliases),src.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::ClearExclusive => out.push_str("    clear_exclusive(&exclusive_monitor);\n"),
                PcodeOp::ClobberExclusive => out.push_str("    exclusive_monitor = unknown_monitor_after_call();\n"),
                PcodeOp::ClobberSimd => out.push_str("    clobber_simd_callersaved(); // V0-V7,V16-V31 and high64(V8-V15); low64(V8-V15) preserved\n"),
                PcodeOp::UnverifiedContinuation{target} => {reg_consts.clear(); out.push_str(&format!("    // INCOMPLETE: fall-through to 0x{target:x} crosses a declared function/section boundary; continuation unverified\n"));},
                PcodeOp::MoveKeep{dst,old,immediate,shift} => out.push_str(&format!("    {} = arm64_movk_u{}({}, 0x{immediate:x}, {shift}); // NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases),dst.size*8,old.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::ConditionalSelect{dst,if_true,if_false,condition} => out.push_str(&format!("    {} = select_u{}({}, {}, {}); // NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases),dst.size*8,condition.expression(),if_true.display_arm64_with_aliases(reg_aliases),if_false.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::SignExtend{dst,src} => out.push_str(&format!("    {} = sign_extend_i{}({});\n",dst.display_arm64_with_aliases(reg_aliases),src.size*8,src.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::LoadAcquire{dst,addr} => out.push_str(&format!("    {} = atomic_load_acquire_u{}({}); // acquire ordering; NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases),dst.size*8,addr.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::ReadThreadPointer{dst} => out.push_str(&format!("    {} = read_thread_pointer(TPIDR_EL0); // runtime per-thread value; NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::NoReturnCall{target,symbol} => out.push_str(&format!("    call_noreturn({symbol:?}, stub=0x{target:x});\n")),
                PcodeOp::ShiftedAdd{dst,lhs,rhs,shift,amount}=>{
                    if let Some(dst)=dst {reg_consts.remove(&dst.offset);}
                    let result=dst.map(|dst|dst.display_arm64_with_aliases(reg_aliases)).unwrap_or_else(||"discard".into());
                    out.push_str(&format!("    {result} = arm64_add_u{}({}, arm64_shift_{:?}_u{}({}, {})); // NZCV preserved\n",lhs.size*8,lhs.display_arm64_with_aliases(reg_aliases),shift,rhs.size*8,rhs.display_arm64_with_aliases(reg_aliases),amount));
                }
                PcodeOp::RegisterOffset{dst,base,index,extend,shift}=>{
                    out.push_str(&format!("    {} = arm64_reg_offset_{:?}({}, {}, {});\n",dst.display_arm64_with_aliases(reg_aliases),extend,base.display_arm64_with_aliases(reg_aliases),index.display_arm64_with_aliases(reg_aliases),shift));
                }
                PcodeOp::TestBitBranch{value,bit,nonzero,target}=>{
                    reg_consts.clear();
                    out.push_str(&format!("    if ((({} >> {}) & 1) {} 0) goto loc_{target:x}; // NZCV preserved\n",value.display_arm64_with_aliases(reg_aliases),bit,if *nonzero {"!="}else{"=="}));
                }
                PcodeOp::IntOr{dst,lhs,rhs,shift,amount}=>{
                    if dst.space==SpaceKind::Register {reg_consts.remove(&dst.offset);}
                    out.push_str(&format!("    {} = {} | arm64_shift_{:?}_u{}({}, {}); // NZCV preserved\n",dst.display_arm64_with_aliases(reg_aliases),lhs.display_arm64_with_aliases(reg_aliases),shift,rhs.size*8,rhs.display_arm64_with_aliases(reg_aliases),amount));
                }
                PcodeOp::FlagArithmetic{dst,lhs,rhs,shift,amount,subtract} => {
                    if let Some(dst)=dst {reg_consts.remove(&dst.offset);}
                    let result=dst.map(|dst|dst.display_arm64_with_aliases(reg_aliases)).unwrap_or_else(||"discard".into());
                    out.push_str(&format!("    ({result}, nzcv) = arm64_{}s_u{}({}, arm64_shift_{:?}_u{}({}, {}));\n",if *subtract {"sub"}else{"add"},lhs.size*8,lhs.display_arm64_with_aliases(reg_aliases),shift,rhs.size*8,rhs.display_arm64_with_aliases(reg_aliases),amount));
                }
                PcodeOp::FlagBranch{condition,target}=>{
                    reg_consts.clear();
                    out.push_str(&format!("    if ({}) goto loc_{target:x};\n",condition.expression()));
                }
                PcodeOp::ClobberNzcv=>out.push_str("    nzcv = unknown_nzcv_after_call();\n"),
                PcodeOp::Bitfield{dst,src,old_dst,kind,immr,imms} => {
                    if dst.space==SpaceKind::Register {reg_consts.remove(&dst.offset);}
                    out.push_str(&format!("    {} = arm64_{:?}_bitfield_u{}({}, {}, {}, {});\n",dst.display_arm64_with_aliases(reg_aliases),kind,dst.size*8,src.display_arm64_with_aliases(reg_aliases),old_dst.display_arm64_with_aliases(reg_aliases),immr,imms));
                }
                PcodeOp::ExternalTailCall{symbol,got_address} => out.push_str(&format!("    tailcall_external({symbol:?}, got=0x{got_address:x}); // JUMP_SLOT symbol resolved; runtime address and signature unknown\n")),
                PcodeOp::MachOImportTailCall{symbol,library,pointer_address,selector_address,selector} => {
                    if let Some(selref)=selector_address {
                        if let Some(name)=selector {out.push_str(&format!("    // verified selector {name:?} from selref 0x{selref:x}; runtime SEL value unknown\n"));}
                        else {out.push_str(&format!("    // selector loaded from selref 0x{selref:x}; name and runtime value unresolved\n"));}
                    }
                    out.push_str(&format!("    tailcall_import({symbol:?}, dylib={library:?}, slot=0x{pointer_address:x}); // Mach-O indirect symbol binding; runtime address and signature unknown\n"));
                },
                PcodeOp::Bti { kind } => out.push_str(&format!(
                    "    bti_guard({}); // runtime branch-target protection\n",
                    kind
                )),
                PcodeOp::Load { dst, addr } => {
                    if dst.space == SpaceKind::Register {
                        reg_consts.remove(&dst.offset);
                    }
                    out.push_str(&format!(
                        "    {} = load_u{}({});\n",
                        dst.display_arm64_with_aliases(reg_aliases),
                        dst.size as u16 * 8,
                        addr.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                PcodeOp::Store { addr, src } => out.push_str(&format!(
                    "    store_u{}({}, {});\n",
                    src.size as u16 * 8,
                    addr.display_arm64_with_aliases(reg_aliases),
                    src.display_arm64_with_aliases(reg_aliases)
                )),
                PcodeOp::ZeroExtend { dst, src } => {
                    if dst.space == SpaceKind::Register {
                        reg_consts.remove(&dst.offset);
                    }
                    out.push_str(&format!(
                        "    {} = zero_extend_u{}({});\n",
                        dst.display_arm64_with_aliases(reg_aliases),
                        src.size as u16 * 8,
                        src.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                PcodeOp::CallInd { target } => {
                    reg_consts.retain(|reg, _| *reg > 18);
                    out.push_str(&format!(
                        "    call_indirect({}); // signature unknown\n",
                        target.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                PcodeOp::RuntimeTailCall { target_reg, target } => {
                    reg_consts.retain(|reg, _| *reg > 18);
                    out.push_str(&format!("    tailcall_runtime(0x{target:x}, via={}); // supplied entry-argument context only; signature unknown; native return allowed\n",target_reg.display_arm64_with_aliases(reg_aliases)));
                }
                PcodeOp::RuntimeInvalidBranch { target_reg, target, signal } => out.push_str(&format!("    runtime_branch_fault(target=0x{target:x}, via={}, signal={signal}); // observed fault, not callable; INCOMPLETE retained\n",target_reg.display_arm64_with_aliases(reg_aliases))),
                PcodeOp::RuntimeJniCall { target_reg, table, target, slot, tail } => {
                    reg_consts.retain(|reg, _| *reg > 18);
                    let operation = crate::jni_runtime::operation(*slot).unwrap_or("unknown");
                    out.push_str(&format!("    {}({operation:?}, via={}, table=0x{table:x}, target=0x{target:x}); // captured JNI slot {slot}; pending exception possible; native return allowed\n", if *tail {"tailcall_runtime_jni"} else {"call_runtime_jni"},target_reg.display_arm64_with_aliases(reg_aliases)));
                }
                PcodeOp::Copy { dst, src } => {
                    if dst.space == SpaceKind::Register {
                        reg_consts.remove(&dst.offset);
                    }
                    if dst.space == SpaceKind::Register && src.space == SpaceKind::Constant {
                        reg_consts.insert(dst.offset, src.offset);
                    }
                    out.push_str(&format!(
                        "    {} = {};\n",
                        dst.display_arm64_with_aliases(reg_aliases),
                        src.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                PcodeOp::IntAdd { dst, lhs, rhs } => {
                    let previous = reg_consts.get(&lhs.offset).copied();
                    if dst.space == SpaceKind::Register {
                        reg_consts.remove(&dst.offset);
                    }
                    if dst.space == SpaceKind::Register
                        && lhs.space == SpaceKind::Register
                        && rhs.space == SpaceKind::Constant
                    {
                        if let Some(base) = previous {
                            let resolved = base.wrapping_add(rhs.offset)
                                & if dst.size == 4 {
                                    u32::MAX as u64
                                } else {
                                    u64::MAX
                                };
                            reg_consts.insert(dst.offset, resolved);
                            if let Some(s) = read_ascii_at(seg, resolved) {
                                out.push_str(&format!(
                                    "    {} = \"{}\"; // 0x{:x}\n",
                                    dst.display_arm64_with_aliases(reg_aliases),
                                    s,
                                    resolved
                                ));
                                continue;
                            }
                        }
                    }
                    out.push_str(&format!(
                        "    {} = {} + {};\n",
                        dst.display_arm64_with_aliases(reg_aliases),
                        lhs.display_arm64_with_aliases(reg_aliases),
                        rhs.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                PcodeOp::IntSub { dst, lhs, rhs } => {
                    reg_consts.remove(&dst.offset);
                    out.push_str(&format!(
                        "    {} = {} - {};\n",
                        dst.display_arm64_with_aliases(reg_aliases),
                        lhs.display_arm64_with_aliases(reg_aliases),
                        rhs.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                PcodeOp::CBranch {
                    cond,
                    target,
                    branch_on_zero,
                } => {
                    reg_consts.clear();
                    out.push_str(&format!(
                        "    if ({}{}) goto loc_{:x};\n",
                        cond.display_arm64_with_aliases(reg_aliases),
                        if *branch_on_zero { " == 0" } else { "" },
                        target
                    ));
                }
                PcodeOp::Branch { target } => {
                    reg_consts.clear();
                    out.push_str(&format!("    goto loc_{:x};\n", target));
                }
                PcodeOp::TableBranch { index, table_address, offset_base, targets, .. } => {
                    reg_consts.clear();
                    out.push_str(&format!("    switch ({}) {{ // verified u16 table 0x{table_address:x}, base 0x{offset_base:x}, scale 4; {} entries\n", index.display_arm64_with_aliases(reg_aliases), targets.len()));
                    for (idx, target) in targets.iter().enumerate() {
                        out.push_str(&format!("        case {idx}: goto loc_{target:x};\n"));
                    }
                    out.push_str("    }\n");
                }
                PcodeOp::BranchInd {
                    target_reg,
                    switch_targets,
                } => {
                    if switch_targets.is_empty() {
                        out.push_str(&format!(
                            "    goto *{};\n",
                            target_reg.display_arm64_with_aliases(reg_aliases)
                        ));
                    } else {
                        out.push_str(&format!(
                            "    switch ({}) {{ // Hopper SwitchHint recovered {} cases\n",
                            target_reg.display_arm64_with_aliases(reg_aliases),
                            switch_targets.len()
                        ));
                        for (idx, t) in switch_targets.iter().enumerate() {
                            out.push_str(&format!("        case {}: goto loc_{:x};\n", idx, t));
                        }
                        out.push_str("    }\n");
                    }
                }
                PcodeOp::CallObjC { selector, .. } => {
                    // Hopper's signature strength: readable Objective-C message syntax!
                    let recv_name = reg_aliases.get(&0).map(String::as_str).unwrap_or("x0");
                    let arg_preview = reg_consts
                        .get(&2)
                        .and_then(|&a| read_ascii_at(seg, a))
                        .map(|s| format!("{s:?}"))
                        .unwrap_or_else(|| {
                            reg_aliases
                                .get(&2)
                                .cloned()
                                .unwrap_or_else(|| "x2".to_string())
                        });
                    let args: Vec<String> = (0..selector.matches(':').count())
                        .map(|i| {
                            if i == 0 {
                                arg_preview.clone()
                            } else if i < 6 {
                                reg_aliases
                                    .get(&(i as u64 + 2))
                                    .cloned()
                                    .unwrap_or_else(|| format!("x{}", i + 2))
                            } else {
                                format!("unknown_stack_arg_{i}")
                            }
                        })
                        .collect();
                    out.push_str(&format!(
                        "    {recv_name} = {};\n",
                        format_objc_message(recv_name, selector, &args)
                    ));
                    reg_consts.retain(|reg, _| *reg > 18);
                }
                PcodeOp::Call { target, symbol } => {
                    reg_consts.retain(|reg, _| *reg > 18);
                    let name = symbol
                        .clone()
                        .unwrap_or_else(|| format!("sub_{:x}", target));
                    out.push_str(&format!("    x0 = {}(x0, x1);\n", name));
                }
                PcodeOp::Return { retval } => {
                    out.push_str(&format!(
                        "    return {};\n",
                        retval.display_arm64_with_aliases(reg_aliases)
                    ));
                }
                _ => {}
            }
        }
    }

    out.push_str("}\n");
    out
}

pub fn read_ascii_at(seg: &Segment, addr: u64) -> Option<String> {
    let available = seg
        .length()
        .checked_sub(addr.checked_sub(seg.start_address)?)?;
    let bytes = seg.read_bytes(addr, available.min(4096) as usize)?;
    let end = bytes.iter().position(|&b| b == 0)?;
    if end == 0 || !bytes[..end].iter().all(|&b| (0x20..=0x7e).contains(&b)) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes[..end]).to_string())
}

pub fn format_objc_message(receiver: &str, selector: &str, args: &[String]) -> String {
    if !selector.contains(':') {
        return format!("[{receiver} {selector}]");
    }
    let parts: Vec<_> = selector.split(':').collect();
    let pairs: Vec<_> = parts[..parts.len() - 1]
        .iter()
        .enumerate()
        .map(|(i, part)| {
            format!(
                "{}:{}",
                part,
                args.get(i).map(String::as_str).unwrap_or("unknown_arg")
            )
        })
        .collect();
    format!("[{receiver} {}]", pairs.join(" "))
}
