use ghidra_hopper_rs::{
    arm64_flags::{add_shifted, Nzcv, Shift},
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
fn decode(w: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        w,
        &Segment::new("text", 0x1000, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}
fn word(wide: bool, rn: u32, rm: u32, rd: u32) -> u32 {
    0x0b000000 | ((wide as u32) << 31) | (rm << 16) | (rn << 5) | rd
}

#[test]
fn hopper_instruction_is_add_with_shift_two_and_no_flag_write() {
    let d = decode(0x8b0a0929);
    assert_eq!(d.mnemonic, "add");
    assert!(
        matches!(d.pcode[0],PcodeOp::ShiftedAdd{dst:Some(dst),lhs,rhs,shift:Shift::Lsl,amount:2} if dst.offset==9 && lhs.offset==9 && rhs.offset==10 && lhs.size==8)
    );
    assert_eq!(d.pcode.len(), 1);
    assert_eq!(
        add_shifted(0x100003268, 7, 64, Shift::Lsl, 2),
        Some(0x100003284)
    );
}

#[test]
fn every_shift_and_width_match_independent_bit_and_widened_sum_oracle() {
    for width in [32u8, 64] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            u32::MAX as u64
        };
        let sign = 1u64 << (width - 1);
        for a in [0, 1, mask, sign, 0x123456789abcdef0] {
            for b in [0, 1, mask, sign, 0x8123456789abcdef] {
                for shift in [Shift::Lsl, Shift::Lsr, Shift::Asr] {
                    for amount in 0..width {
                        let mut operand = 0;
                        for output in 0..width {
                            let input = match shift {
                                Shift::Lsl => output.checked_sub(amount),
                                Shift::Lsr => output.checked_add(amount).filter(|i| *i < width),
                                Shift::Asr => Some(output.saturating_add(amount).min(width - 1)),
                            };
                            if input.is_some_and(|i| b & (1u64 << i) != 0) {
                                operand |= 1u64 << output;
                            }
                        }
                        assert_eq!(
                            add_shifted(a, b, width, shift, amount),
                            Some(((a as u128 + operand as u128) & mask as u128) as u64)
                        );
                    }
                }
            }
        }
    }
    assert_eq!(add_shifted(u64::MAX, 1, 64, Shift::Lsl, 0), Some(0));
    assert_eq!(
        add_shifted(u64::MAX, 0, 32, Shift::Lsl, 0),
        Some(u32::MAX as u64)
    );
}

#[test]
fn register_31_is_zero_or_discard_never_sp() {
    for wide in [false, true] {
        for rn in [0, 1, 30, 31] {
            for rm in [0, 1, 31] {
                for rd in [0, 1, 31] {
                    match decode(word(wide, rn, rm, rd)).pcode[0] {
                        PcodeOp::ShiftedAdd { dst, lhs, rhs, .. } => {
                            assert_eq!(
                                lhs.space,
                                if rn == 31 {
                                    SpaceKind::Constant
                                } else {
                                    SpaceKind::Register
                                }
                            );
                            assert_eq!(lhs.offset, if rn == 31 { 0 } else { rn as u64 });
                            assert_eq!(
                                rhs.space,
                                if rm == 31 {
                                    SpaceKind::Constant
                                } else {
                                    SpaceKind::Register
                                }
                            );
                            assert_eq!(rhs.offset, if rm == 31 { 0 } else { rm as u64 });
                            assert_eq!(dst.is_none(), rd == 31);
                            assert_eq!(lhs.size, if wide { 8 } else { 4 });
                        }
                        _ => panic!("ADD missing"),
                    }
                }
            }
        }
    }
}

#[test]
fn flag_setting_immediate_and_reserved_forms_are_distinct() {
    assert!(matches!(
        decode(0xab010000).pcode[0],
        PcodeOp::FlagArithmetic { .. }
    ));
    assert!(matches!(
        decode(0x91000400).pcode[0],
        PcodeOp::IntAdd { .. }
    ));
    for w in [0x0b018000, 0x8bc10000, 0x8b211400, 0xcbc10000] {
        assert!(
            matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }),
            "{w:x}"
        );
    }
    assert_eq!(add_shifted(1, 2, 32, Shift::Lsl, 32), None);
    assert_eq!(add_shifted(1, 2, 16, Shift::Lsl, 0), None);
}

#[test]
fn aliased_destination_is_written_after_inputs_and_flags_are_preserved() {
    for w in [word(false, 0, 1, 0), word(false, 0, 1, 1)] {
        for bits in 0..16 {
            let before = Nzcv::from_bits(bits);
            let mut flags = Some(before);
            let mut regs = [0u64; 32];
            regs[0] = 0xffffffff;
            regs[1] = 2;
            let mut destination = 0;
            for op in decode(w).pcode {
                match op {
                    PcodeOp::ShiftedAdd {
                        dst: Some(dst),
                        lhs,
                        rhs,
                        shift,
                        amount,
                    } => {
                        let value = add_shifted(
                            regs[lhs.offset as usize],
                            regs[rhs.offset as usize],
                            lhs.size * 8,
                            shift,
                            amount,
                        )
                        .unwrap();
                        destination = dst.offset as usize;
                        regs[destination] = value;
                    }
                    PcodeOp::ClobberNzcv => flags = None,
                    _ => panic!("ordinary ADD must not set NZCV"),
                }
            }
            assert_eq!(regs[destination], 1);
            assert_eq!(flags, Some(before));
        }
    }
}

#[test]
fn shifted_add_invalidates_old_string_argument() {
    let words = [0x90000002u32, 0x91010042, 0x8b030842, 0x94000004];
    let mut bytes = vec![0; 128];
    for (i, w) in words.into_iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    bytes[64..68].copy_from_slice(b"old\0");
    let mut p = GhidraProgram::new("test", Segment::new("text", 0x1000, 0, bytes));
    p.objc_stub_selectors.insert(0x101c, "go:".into());
    let mut a = HopperHybridObjcAnalyzerRs::new();
    a.analyze_function_zero_ipc(&mut p, "test", 0x1000, 4, &[]);
    assert_eq!(a.discovered_messages[0].parameters, vec!["x2"]);
}
