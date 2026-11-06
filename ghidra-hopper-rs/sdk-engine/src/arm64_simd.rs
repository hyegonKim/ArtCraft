//! Bitmask MOVI and SIMD B/H/S/D/Q transfers. Vector indices are
//! deliberately separate from GPR Varnodes, including V31 versus SP/ZR.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode};
pub fn expanded_bytes(immediate: u8) -> u64 {
    (0..8).fold(0, |out, n| {
        out | if immediate & (1 << n) != 0 {
            255u64 << (8 * n)
        } else {
            0
        }
    })
}
/// Integer MOVI constants only: other modified-immediate operations stay unsupported.
pub fn movi_immediate(op: bool, cmode: u8, immediate: u8) -> Option<u64> {
    if op {
        return (cmode == 14).then(|| expanded_bytes(immediate));
    }
    let (element, bits) = match cmode {
        0 | 2 | 4 | 6 => ((immediate as u64) << (4 * cmode), 32),
        8 | 10 => ((immediate as u64) << (4 * (cmode - 8)), 16),
        12 | 13 => {
            let shift = 8 * (cmode - 11);
            (((immediate as u64) << shift) | ((1u64 << shift) - 1), 32)
        }
        14 => (immediate as u64, 8),
        _ => return None,
    };
    Some((0..64 / bits).fold(0, |value, lane| value | (element << (lane * bits))))
}
fn temp(offset: u64) -> Varnode {
    Varnode {
        space: SpaceKind::Unique,
        offset,
        size: 8,
    }
}
pub fn decode(address: u64, word: u32) -> Option<DecodedInstruction> {
    let mut i = DecodedInstruction {
        address,
        bytes: word.to_le_bytes(),
        mnemonic: String::new(),
        raw_args: vec![],
        formatted_args: vec![],
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![],
    };
    if word & 0x3f200c00 == 0x3c200800 {
        let scale = (word >> 30) & 3;
        let opc = (word >> 22) & 3;
        if opc >= 2 && scale != 0 {
            return None;
        }
        let size = if opc >= 2 { 16 } else { 1u8 << scale };
        let load = opc & 1 != 0;
        let (extend, index_size, modifier) = match (word >> 13) & 7 {
            2 => (crate::arm64_indexed::IndexExtend::Uxtw, 4, "uxtw"),
            3 => (crate::arm64_indexed::IndexExtend::Uxtx, 8, "lsl"),
            6 => (crate::arm64_indexed::IndexExtend::Sxtw, 4, "sxtw"),
            7 => (crate::arm64_indexed::IndexExtend::Sxtx, 8, "sxtx"),
            _ => return None,
        };
        let shift = if word & (1 << 12) != 0 {
            size.trailing_zeros() as u8
        } else {
            0
        };
        let rn = ((word >> 5) & 31) as u64;
        let rm = ((word >> 16) & 31) as u64;
        let register = (word & 31) as u8;
        let base = Varnode::reg(rn, 8);
        let index = if rm == 31 {
            Varnode::imm(0, index_size)
        } else {
            Varnode::reg(rm, index_size)
        };
        let index_name = if rm == 31 {
            if index_size == 4 { "wzr" } else { "xzr" }.into()
        } else {
            index.display_arm64()
        };
        let prefix = match size {
            1 => "b",
            2 => "h",
            4 => "s",
            8 => "d",
            _ => "q",
        };
        i.mnemonic = if load { "ldr" } else { "str" }.into();
        i.raw_args = vec![
            format!("{prefix}{register}"),
            format!(
                "[{}, {index_name}, {modifier} #{shift}]",
                base.display_arm64()
            ),
        ];
        i.pcode = vec![PcodeOp::RegisterOffset {
            dst: temp(0),
            base,
            index,
            extend,
            shift,
        }];
        i.pcode.push(if size == 16 {
            if load {
                PcodeOp::VectorLoad {
                    register,
                    addr: temp(0),
                }
            } else {
                PcodeOp::VectorStore {
                    register,
                    addr: temp(0),
                }
            }
        } else if load {
            PcodeOp::VectorLoadSized {
                register,
                addr: temp(0),
                size,
            }
        } else {
            PcodeOp::VectorStoreSized {
                register,
                addr: temp(0),
                size,
            }
        });
    } else if word & 0x9ff80c00 == 0x0f000400 {
        let register = (word & 31) as u8;
        let full = word & (1 << 30) != 0;
        let op = word & (1 << 29) != 0;
        let cmode = ((word >> 12) & 15) as u8;
        let imm = ((((word >> 16) & 7) << 5) | ((word >> 5) & 31)) as u8;
        let low = movi_immediate(op, cmode, imm)?;
        let high = if full { low } else { 0 };
        i.mnemonic = "movi".into();
        if op {
            i.raw_args = vec![
                if full {
                    format!("v{register}.2d")
                } else {
                    format!("d{register}")
                },
                format!("#0x{low:016x}"),
            ];
        } else {
            let (bits, modifier, shift) = match cmode {
                14 => (8, "lsl", 0),
                8 | 10 => (16, "lsl", 4 * (cmode - 8)),
                12 | 13 => (32, "msl", 8 * (cmode - 11)),
                _ => (32, "lsl", 4 * cmode),
            };
            let suffix = if bits == 8 {
                "b"
            } else if bits == 16 {
                "h"
            } else {
                "s"
            };
            i.raw_args = vec![
                format!(
                    "v{register}.{}{suffix}",
                    if full { 128 / bits } else { 64 / bits }
                ),
                format!("#0x{imm:x}"),
            ];
            if shift != 0 {
                i.raw_args.push(format!("{modifier} #{shift}"));
            }
        }
        i.pcode = vec![PcodeOp::VectorImmediate {
            register,
            low,
            high,
        }];
    } else if word & 0x3e000000 == 0x2c000000 && word >> 30 <= 2 {
        let size = 4u8 << (word >> 30);
        let mode = (word >> 23) & 3;
        let load = word & (1 << 22) != 0;
        let first = (word & 31) as u8;
        let second = ((word >> 10) & 31) as u8;
        if mode == 0 || (load && first == second) {
            return None;
        }
        let rn = ((word >> 5) & 31) as u64;
        let base = Varnode::reg(rn, 8);
        let displacement = ((((word >> 15) & 127) as i64)
            - if word & (1 << 21) != 0 { 128 } else { 0 })
            * size as i64;
        i.mnemonic = if load { "ldp" } else { "stp" }.into();
        let memory = match mode {
            1 => format!("[{}], #{displacement}", base.display_arm64()),
            3 => format!("[{}, #{displacement}]!", base.display_arm64()),
            _ => format!("[{}, #{displacement}]", base.display_arm64()),
        };
        let prefix = match size {
            4 => "s",
            8 => "d",
            _ => "q",
        };
        i.raw_args = vec![
            format!("{prefix}{first}"),
            format!("{prefix}{second}"),
            memory,
        ];
        i.pcode = vec![
            PcodeOp::IntAdd {
                dst: temp(0),
                lhs: base,
                rhs: Varnode::imm(if mode == 1 { 0 } else { displacement as u64 }, 8),
            },
            PcodeOp::IntAdd {
                dst: temp(1),
                lhs: temp(0),
                rhs: Varnode::imm(size as u64, 8),
            },
        ];
        for (register, addr) in [(first, temp(0)), (second, temp(1))] {
            i.pcode.push(if size != 16 {
                if load {
                    PcodeOp::VectorLoadSized {
                        register,
                        addr,
                        size,
                    }
                } else {
                    PcodeOp::VectorStoreSized {
                        register,
                        addr,
                        size,
                    }
                }
            } else if load {
                PcodeOp::VectorLoad { register, addr }
            } else {
                PcodeOp::VectorStore { register, addr }
            });
        }
        if mode != 2 {
            i.pcode.push(PcodeOp::IntAdd {
                dst: base,
                lhs: base,
                rhs: Varnode::imm(displacement as u64, 8),
            });
        }
    } else if word & 0x3f000000 == 0x3d000000 || word & 0x3f200000 == 0x3c000000 {
        let unsigned = word & (1 << 24) != 0;
        let scale = (word >> 30) & 3;
        let opc = (word >> 22) & 3;
        if opc >= 2 && scale != 0 {
            return None;
        }
        let size = if opc >= 2 { 16 } else { 1u8 << scale };
        let register = (word & 31) as u8;
        let load = opc & 1 != 0;
        let base = Varnode::reg(((word >> 5) & 31) as u64, 8);
        let mode = if unsigned { 0 } else { (word >> 10) & 3 };
        if mode == 2 {
            return None;
        }
        let displacement = if unsigned {
            (((word >> 10) & 4095) * size as u32) as i64
        } else {
            (((word >> 12) & 511) as i64) - if word & (1 << 20) != 0 { 512 } else { 0 }
        };
        let prefix = match size {
            1 => "b",
            2 => "h",
            4 => "s",
            8 => "d",
            _ => "q",
        };
        i.mnemonic = if load {
            if !unsigned && mode == 0 {
                "ldur"
            } else {
                "ldr"
            }
        } else if !unsigned && mode == 0 {
            "stur"
        } else {
            "str"
        }
        .into();
        let memory = match mode {
            1 => format!("[{}], #{displacement}", base.display_arm64()),
            3 => format!("[{}, #{displacement}]!", base.display_arm64()),
            _ => format!("[{}, #{displacement}]", base.display_arm64()),
        };
        i.raw_args = vec![format!("{prefix}{register}"), memory];
        i.pcode = vec![PcodeOp::IntAdd {
            dst: temp(0),
            lhs: base,
            rhs: Varnode::imm(if mode == 1 { 0 } else { displacement as u64 }, 8),
        }];
        i.pcode.push(if size == 16 {
            if load {
                PcodeOp::VectorLoad {
                    register,
                    addr: temp(0),
                }
            } else {
                PcodeOp::VectorStore {
                    register,
                    addr: temp(0),
                }
            }
        } else if load {
            PcodeOp::VectorLoadSized {
                register,
                addr: temp(0),
                size,
            }
        } else {
            PcodeOp::VectorStoreSized {
                register,
                addr: temp(0),
                size,
            }
        });
        if matches!(mode, 1 | 3) {
            i.pcode.push(PcodeOp::IntAdd {
                dst: base,
                lhs: base,
                rhs: Varnode::imm(displacement as u64, 8),
            });
        }
    } else {
        return None;
    }
    i.formatted_args = i.raw_args.clone();
    Some(i)
}
