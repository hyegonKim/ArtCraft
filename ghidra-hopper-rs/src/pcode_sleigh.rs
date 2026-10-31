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
                31 => "sp".to_string(),
                30 => "lr".to_string(),
                29 => "fp".to_string(),
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
}

/// Core subset of Ghidra P-Code opcodes (`OpCode`), augmented with `CallObjC` for Hopper parity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcodeOp {
    Copy { dst: Varnode, src: Varnode },
    IntAdd { dst: Varnode, lhs: Varnode, rhs: Varnode },
    IntSub { dst: Varnode, lhs: Varnode, rhs: Varnode },
    IntAnd { dst: Varnode, lhs: Varnode, rhs: Varnode },
    IntEqual { dst: Varnode, lhs: Varnode, rhs: Varnode },
    Load { dst: Varnode, addr: Varnode },
    Store { addr: Varnode, src: Varnode },
    Branch { target: u64 },
    CBranch { cond: Varnode, target: u64 },
    BranchInd { target_reg: Varnode, switch_targets: Vec<u64> },
    Call { target: u64, symbol: Option<String> },
    CallObjC { stub_addr: u64, selector: String },
    Return { retval: Varnode },
    Nop,
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

        let sym = seg.get_demangled_name_at_address(target).map(str::to_string);
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
        let reg_vn = Varnode::reg(rt, if is_64 { 8 } else { 4 });
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
            }],
        };
    }

    // 6. B.<cond> <imm19> (0x54000000)
    if (word & 0xff000010) == 0x54000000 {
        let cond_code = word & 0xf;
        let cond_name = match cond_code {
            0 => "eq",
            1 => "ne",
            2 => "cs",
            3 => "cc",
            8 => "hi",
            9 => "ls",
            10 => "ge",
            11 => "lt",
            12 => "gt",
            13 => "le",
            _ => "al",
        };
        let imm19 = ((word >> 5) & 0x7ffff) as i32;
        let signed_imm = ((imm19 << 13) >> 13) as i64 * 4;
        let target = (addr as i64).wrapping_add(signed_imm) as u64;
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: format!("b.{}", cond_name),
            raw_args: vec![format!("0x{:x}", target)],
            formatted_args: vec![fmt_arg(0, target)],
            is_conditional_jump: true,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: false,
            branch_target: Some(target),
            call_ref: None,
            pcode: vec![PcodeOp::CBranch {
                cond: Varnode::reg(100 + cond_code as u64, 1),
                target,
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
    if (word & 0x7f800000) == 0x52800000 {
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
    if (word & 0x1f000000) == 0x11000000 {
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

    // 10. ADRP Xd, <page>
    if (word & 0x9f000000) == 0x90000000 {
        let rd = (word & 0x1f) as u64;
        let immlo = ((word >> 29) & 0x3) as i64;
        let immhi = ((word >> 5) & 0x7ffff) as i64;
        let raw_pages = (immhi << 2) | immlo;
        let signed_pages = (raw_pages << 43) >> 43;
        let page_base = ((addr & !0xfff) as i64).wrapping_add(signed_pages << 12) as u64;
        let dst = Varnode::reg(rd, 8);
        return DecodedInstruction {
            address: addr,
            bytes,
            mnemonic: "adrp".into(),
            raw_args: vec![dst.display_arm64(), format!("0x{:x}", page_base)],
            formatted_args: vec![dst.display_arm64(), fmt_arg(1, page_base)],
            is_conditional_jump: false,
            is_unconditional_jump: false,
            is_call: false,
            is_ret: false,
            branch_target: None,
            call_ref: None,
            pcode: vec![PcodeOp::Copy {
                dst,
                src: Varnode::imm(page_base, 8),
            }],
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
        pcode: vec![PcodeOp::Nop],
    }
}

/// Hybrid Ghidra P-Code + Hopper Objective-C Structured Decompiler.
/// Folds register constants/ADRP+ADD pairs and emits clean C / Objective-C pseudocode.
pub fn decompile_instructions_to_pseudocode(
    proc_name: &str,
    entry_addr: u64,
    insns: &[DecodedInstruction],
    seg: &Segment,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Ghidra-Hopper Hybrid Decompiler (P-Code SSA + ObjC Metadata)\nint64_t {}() {{ // @ 0x{:x}\n",
        proc_name, entry_addr
    ));

    let mut reg_consts: BTreeMap<u64, u64> = BTreeMap::new();

    for insn in insns {
        for op in &insn.pcode {
            match op {
                PcodeOp::Copy { dst, src } => {
                    if dst.space == SpaceKind::Register && src.space == SpaceKind::Constant {
                        reg_consts.insert(dst.offset, src.offset);
                    }
                    out.push_str(&format!(
                        "    {} = {};\n",
                        dst.display_arm64(),
                        src.display_arm64()
                    ));
                }
                PcodeOp::IntAdd { dst, lhs, rhs } => {
                    if dst.space == SpaceKind::Register
                        && lhs.space == SpaceKind::Register
                        && rhs.space == SpaceKind::Constant
                    {
                        if let Some(&base) = reg_consts.get(&lhs.offset) {
                            let resolved = base.wrapping_add(rhs.offset);
                            reg_consts.insert(dst.offset, resolved);
                            if let Some(s) = read_ascii_at(seg, resolved) {
                                out.push_str(&format!(
                                    "    {} = \"{}\"; // 0x{:x}\n",
                                    dst.display_arm64(),
                                    s,
                                    resolved
                                ));
                                continue;
                            }
                        }
                    }
                    out.push_str(&format!(
                        "    {} = {} + {};\n",
                        dst.display_arm64(),
                        lhs.display_arm64(),
                        rhs.display_arm64()
                    ));
                }
                PcodeOp::IntSub { dst, lhs, rhs } => {
                    out.push_str(&format!(
                        "    {} = {} - {};\n",
                        dst.display_arm64(),
                        lhs.display_arm64(),
                        rhs.display_arm64()
                    ));
                }
                PcodeOp::CBranch { cond, target } => {
                    out.push_str(&format!(
                        "    if ({}) goto loc_{:x};\n",
                        cond.display_arm64(),
                        target
                    ));
                }
                PcodeOp::Branch { target } => {
                    out.push_str(&format!("    goto loc_{:x};\n", target));
                }
                PcodeOp::BranchInd {
                    target_reg,
                    switch_targets,
                } => {
                    if switch_targets.is_empty() {
                        out.push_str(&format!("    goto *{};\n", target_reg.display_arm64()));
                    } else {
                        out.push_str(&format!(
                            "    switch ({}) {{ // Hopper SwitchHint recovered {} cases\n",
                            target_reg.display_arm64(),
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
                    let arg_preview = reg_consts
                        .get(&2)
                        .and_then(|&a| read_ascii_at(seg, a))
                        .map(|s| format!("@\"{}\"", s))
                        .unwrap_or_else(|| "x2".to_string());
                    if selector.contains(':') {
                        out.push_str(&format!("    x0 = [x0 {}{}];\n", selector, arg_preview));
                    } else {
                        out.push_str(&format!("    x0 = [x0 {}];\n", selector));
                    }
                }
                PcodeOp::Call { target, symbol } => {
                    let name = symbol
                        .clone()
                        .unwrap_or_else(|| format!("sub_{:x}", target));
                    out.push_str(&format!("    x0 = {}(x0, x1);\n", name));
                }
                PcodeOp::Return { retval } => {
                    out.push_str(&format!("    return {};\n", retval.display_arm64()));
                }
                _ => {}
            }
        }
    }

    out.push_str("}\n");
    out
}

fn read_ascii_at(seg: &Segment, addr: u64) -> Option<String> {
    let bytes = seg.read_bytes(addr, 64)?;
    let end = bytes.iter().position(|&b| b == 0)?;
    if end == 0 || !bytes[..end].iter().all(|&b| (0x20..=0x7e).contains(&b)) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes[..end]).to_string())
}
