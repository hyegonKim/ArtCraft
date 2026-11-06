//! Integer memory transfers and ABI prologue instructions. Address temporaries
//! snapshot the original base before loads can overwrite it.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode};

fn temp(index: u64, size: u8) -> Varnode {
    Varnode {
        space: SpaceKind::Unique,
        offset: index,
        size,
    }
}
fn read_reg(index: u64, size: u8) -> Varnode {
    if index == 31 {
        Varnode::imm(0, size)
    } else {
        Varnode::reg(index, size)
    }
}
fn write_reg(index: u64, size: u8) -> Varnode {
    if index == 31 {
        temp(2, size)
    } else {
        Varnode::reg(index, size)
    }
}
fn name(index: u64, size: u8) -> String {
    if index == 31 {
        if size == 8 { "xzr" } else { "wzr" }.into()
    } else {
        Varnode::reg(index, size).display_arm64()
    }
}
fn instruction(
    address: u64,
    word: u32,
    mnemonic: &str,
    args: Vec<String>,
    pcode: Vec<PcodeOp>,
) -> DecodedInstruction {
    DecodedInstruction {
        address,
        bytes: word.to_le_bytes(),
        mnemonic: mnemonic.into(),
        raw_args: args.clone(),
        formatted_args: args,
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode,
    }
}

pub fn decode(address: u64, word: u32) -> Option<DecodedInstruction> {
    // BTI, BTI c, BTI j, BTI jc. Keep the guard in IR; do not claim
    // to execute or validate the runtime BTYPE / guarded-page checks.
    if word & !0xc0 == 0xd503241f {
        let kind = ((word >> 6) & 3) as u8;
        let target = ["", "c", "j", "jc"][kind as usize];
        return Some(instruction(
            address,
            word,
            "bti",
            if target.is_empty() {
                vec![]
            } else {
                vec![target.into()]
            },
            vec![PcodeOp::Bti { kind }],
        ));
    }
    // MOV register is ORR Rd, ZR, Rm with LSL #0, not MOV/SP (ADD alias).
    if word & 0x7fe0ffe0 == 0x2a0003e0 {
        let size = if word >> 31 != 0 { 8 } else { 4 };
        let rd = (word & 31) as u64;
        let rm = ((word >> 16) & 31) as u64;
        return Some(instruction(
            address,
            word,
            "mov",
            vec![name(rd, size), name(rm, size)],
            vec![PcodeOp::Copy {
                dst: write_reg(rd, size),
                src: read_reg(rm, size),
            }],
        ));
    }
    if word & 0xfffffc1f == 0xd63f0000 {
        let rn = ((word >> 5) & 31) as u64;
        if rn == 31 {
            return None;
        }
        let mut insn = instruction(
            address,
            word,
            "blr",
            vec![name(rn, 8)],
            vec![PcodeOp::CallInd {
                target: Varnode::reg(rn, 8),
            }],
        );
        insn.is_call = true;
        return Some(insn);
    }
    // Integer LDP/STP, signed scaled imm7: post-index / offset / pre-index.
    // Exclude SIMD, LDPSW, non-temporal, and constrained-unpredictable overlaps.
    if word & 0x3e000000 == 0x28000000 && word & (1 << 26) == 0 {
        let opc = word >> 30;
        let mode = (word >> 23) & 3;
        let load = word & (1 << 22) != 0;
        if !matches!(opc, 0 | 2) || mode == 0 {
            return None;
        }
        let size = if opc == 2 { 8 } else { 4 };
        let rt = (word & 31) as u64;
        let rt2 = ((word >> 10) & 31) as u64;
        let rn = ((word >> 5) & 31) as u64;
        if (load && rt == rt2) || (mode != 2 && rn != 31 && (rn == rt || rn == rt2)) {
            return None;
        }
        let displacement = (((word >> 15) & 127) as i64
            - if word & (1 << 21) != 0 { 128 } else { 0 })
            * size as i64;
        let base = Varnode::reg(rn, 8);
        let a = temp(0, 8);
        let b = temp(1, 8);
        let mut ops = vec![
            PcodeOp::IntAdd {
                dst: a,
                lhs: base,
                rhs: Varnode::imm(if mode == 1 { 0 } else { displacement as u64 }, 8),
            },
            PcodeOp::IntAdd {
                dst: b,
                lhs: a,
                rhs: Varnode::imm(size as u64, 8),
            },
        ];
        for (reg, addr) in [(rt, a), (rt2, b)] {
            ops.push(if load {
                PcodeOp::Load {
                    dst: write_reg(reg, size),
                    addr,
                }
            } else {
                PcodeOp::Store {
                    addr,
                    src: read_reg(reg, size),
                }
            });
        }
        if mode != 2 {
            ops.push(PcodeOp::IntAdd {
                dst: base,
                lhs: base,
                rhs: Varnode::imm(displacement as u64, 8),
            });
        }
        let base_name = base.display_arm64();
        let memory = match mode {
            1 => format!("[{base_name}], #{displacement}"),
            3 => format!("[{base_name}, #{displacement}]!"),
            _ => format!("[{base_name}, #{displacement}]"),
        };
        return Some(instruction(
            address,
            word,
            if load { "ldp" } else { "stp" },
            vec![name(rt, size), name(rt2, size), memory],
            ops,
        ));
    }
    // Integer unsigned immediate LDR/STR, including byte/halfword zero-extending loads.
    if word & 0x3f000000 == 0x39000000 {
        let opc = (word >> 22) & 3;
        let size = 1u8 << ((word >> 30) & 3);
        let signed_word = opc == 2 && size == 4;
        if opc > 1 && !signed_word {
            return None;
        }
        let rt = (word & 31) as u64;
        let rn = ((word >> 5) & 31) as u64;
        let displacement = ((word >> 10) & 4095) as u64 * size as u64;
        let a = temp(0, 8);
        let mut ops = vec![PcodeOp::IntAdd {
            dst: a,
            lhs: Varnode::reg(rn, 8),
            rhs: Varnode::imm(displacement, 8),
        }];
        // Memory width and architectural register write width are distinct for LDRB/H.
        if opc == 1 || signed_word {
            ops.push(PcodeOp::Load {
                dst: temp(1, size),
                addr: a,
            });
            ops.push(if signed_word {
                PcodeOp::SignExtend {
                    dst: write_reg(rt, 8),
                    src: temp(1, 4),
                }
            } else {
                PcodeOp::ZeroExtend {
                    dst: write_reg(rt, if size == 8 { 8 } else { 4 }),
                    src: temp(1, size),
                }
            });
        } else {
            ops.push(PcodeOp::Store {
                addr: a,
                src: read_reg(rt, size),
            });
        }
        let mnemonic = match (opc, size) {
            (2, 4) => "ldrsw",
            (0, 1) => "strb",
            (0, 2) => "strh",
            (0, _) => "str",
            (1, 1) => "ldrb",
            (1, 2) => "ldrh",
            _ => "ldr",
        };
        return Some(instruction(
            address,
            word,
            mnemonic,
            vec![
                name(rt, if size == 8 || signed_word { 8 } else { 4 }),
                format!(
                    "[{}, #{}]",
                    Varnode::reg(rn, 8).display_arm64(),
                    displacement
                ),
            ],
            ops,
        ));
    }
    // Integer unscaled / pre-index / post-index immediate single transfers.
    // Unprivileged encodings (mode 2), signed byte/halfword and SIMD remain unsupported.
    if word & 0x3f200000 == 0x38000000 {
        let opc = (word >> 22) & 3;
        let mode = (word >> 10) & 3;
        let size = 1u8 << ((word >> 30) & 3);
        let signed_word = opc == 2 && size == 4;
        if (opc > 1 && !signed_word) || mode == 2 {
            return None;
        }
        let rt = (word & 31) as u64;
        let rn = ((word >> 5) & 31) as u64;
        if mode != 0 && rn != 31 && rt == rn {
            return None;
        }
        let displacement =
            (((word >> 12) & 511) as i64) - if word & (1 << 20) != 0 { 512 } else { 0 };
        let base = Varnode::reg(rn, 8);
        let a = temp(0, 8);
        let mut ops = vec![PcodeOp::IntAdd {
            dst: a,
            lhs: base,
            rhs: Varnode::imm(if mode == 1 { 0 } else { displacement as u64 }, 8),
        }];
        if opc == 1 || signed_word {
            ops.push(PcodeOp::Load {
                dst: temp(1, size),
                addr: a,
            });
            ops.push(if signed_word {
                PcodeOp::SignExtend {
                    dst: write_reg(rt, 8),
                    src: temp(1, 4),
                }
            } else {
                PcodeOp::ZeroExtend {
                    dst: write_reg(rt, if size == 8 { 8 } else { 4 }),
                    src: temp(1, size),
                }
            });
        } else {
            ops.push(PcodeOp::Store {
                addr: a,
                src: read_reg(rt, size),
            });
        }
        if mode != 0 {
            ops.push(PcodeOp::IntAdd {
                dst: base,
                lhs: base,
                rhs: Varnode::imm(displacement as u64, 8),
            });
        }
        let mnemonic = match (mode == 0, opc, size) {
            (true, 2, 4) => "ldursw",
            (false, 2, 4) => "ldrsw",
            (true, 0, 1) => "sturb",
            (true, 0, 2) => "sturh",
            (true, 0, _) => "stur",
            (true, 1, 1) => "ldurb",
            (true, 1, 2) => "ldurh",
            (true, 1, _) => "ldur",
            (false, 0, 1) => "strb",
            (false, 0, 2) => "strh",
            (false, 0, _) => "str",
            (false, 1, 1) => "ldrb",
            (false, 1, 2) => "ldrh",
            _ => "ldr",
        };
        let memory = match mode {
            1 => format!("[{}], #{displacement}", base.display_arm64()),
            3 => format!("[{}, #{displacement}]!", base.display_arm64()),
            _ => format!("[{}, #{displacement}]", base.display_arm64()),
        };
        return Some(instruction(
            address,
            word,
            mnemonic,
            vec![
                name(rt, if size == 8 || signed_word { 8 } else { 4 }),
                memory,
            ],
            ops,
        ));
    }
    None
}
