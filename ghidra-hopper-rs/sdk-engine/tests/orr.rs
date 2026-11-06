use ghidra_hopper_rs::{
    arm64_logical::{evaluate, immediate_mask, LogicalShift},
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::{BTreeMap, BTreeSet};
fn decode(word: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        word,
        &Segment::new("text", 0x1000, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}

#[test]
fn hopper_orr_has_real_inputs_and_no_flag_write() {
    let insn = decode(0xaa170308);
    assert_eq!(insn.mnemonic, "orr");
    assert!(
        matches!(insn.pcode[0],PcodeOp::IntOr{dst,lhs,rhs,shift:LogicalShift::Lsl,amount:0} if dst.offset==8 && lhs.offset==24 && rhs.offset==23 && dst.size==8)
    );
    assert_eq!(insn.pcode.len(), 1);
    assert_eq!(evaluate(0x50, 0x0f, 64, LogicalShift::Lsl, 0), Some(0x5f));
}

#[test]
fn all_shift_amounts_match_independent_bit_index_oracle() {
    for width in [32u8, 64] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            u32::MAX as u64
        };
        let sign = 1u64 << (width - 1);
        for lhs in [0, 1, mask, sign, 0x123456789abcdef0] {
            for rhs in [0, 1, mask, sign, 0x89abcdef01234567] {
                for shift in [
                    LogicalShift::Lsl,
                    LogicalShift::Lsr,
                    LogicalShift::Asr,
                    LogicalShift::Ror,
                ] {
                    for amount in 0..width {
                        let mut shifted = 0;
                        for output in 0..width {
                            let input = match shift {
                                LogicalShift::Lsl => output.checked_sub(amount),
                                LogicalShift::Lsr => {
                                    output.checked_add(amount).filter(|i| *i < width)
                                }
                                LogicalShift::Asr => {
                                    Some(output.saturating_add(amount).min(width - 1))
                                }
                                LogicalShift::Ror => Some((output + amount) % width),
                            };
                            if input.is_some_and(|i| rhs & (1u64 << i) != 0) {
                                shifted |= 1u64 << output;
                            }
                        }
                        assert_eq!(
                            evaluate(lhs, rhs, width, shift, amount),
                            Some((lhs | shifted) & mask)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn logical_immediates_exactly_cover_repeated_rotated_nontrivial_runs() {
    assert_eq!(immediate_mask(64, 1, 1, 0), Some(0x8000000000000000));
    assert_eq!(immediate_mask(64, 1, 5, 7), Some(0xf800000000000007));
    assert_eq!(immediate_mask(32, 0, 5, 7), Some(0xf8000007));
    assert_eq!(immediate_mask(64, 0, 1, 48), Some(0x8080808080808080));
    assert_eq!(immediate_mask(64, 0, 9, 48), Some(0x8080808080808080));
    assert_eq!(immediate_mask(32, 0, 32, 0), Some(1));
    for width in [32u8, 64] {
        let mut expected = BTreeSet::new();
        for element in [2u8, 4, 8, 16, 32, 64].into_iter().filter(|e| *e <= width) {
            for count in 1..element {
                for rotation in 0..element {
                    let mut value = 0;
                    for bit in 0..width {
                        if (bit + rotation) % element < count {
                            value |= 1u64 << bit;
                        }
                    }
                    expected.insert(value);
                }
            }
        }
        let mut actual = BTreeSet::new();
        for n in 0..2 {
            for r in 0..64 {
                for s in 0..64 {
                    if let Some(value) = immediate_mask(width, n, r, s) {
                        actual.insert(value);
                    }
                }
            }
        }
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), if width == 32 { 1302 } else { 5334 });
        assert_eq!(immediate_mask(width, 0, 0, 63), None);
    }
}

#[test]
fn zero_register_sp_and_mov_alias_are_not_confused() {
    assert!(matches!(decode(0xaa0103e0).pcode[0], PcodeOp::Copy { .. })); // existing MOV alias
    let reg = decode(0xaa01001f); // orr xzr,x0,x1
    assert!(matches!(reg.pcode[0],PcodeOp::IntOr{dst,..} if dst.space==SpaceKind::Unique));
    let source = decode(0xaa1f0000); // orr x0,x0,xzr
    assert!(
        matches!(source.pcode[0],PcodeOp::IntOr{rhs,..} if rhs.space==SpaceKind::Constant && rhs.offset==0)
    );
    let immediate = decode(0xb240001f); // orr sp,x0,#1
    assert!(
        matches!(immediate.pcode[0],PcodeOp::IntOr{dst,rhs,..} if dst.space==SpaceKind::Register && dst.offset==31 && rhs.offset==1)
    );
    let source = decode(0xb24003e0); // orr x0,xzr,#1
    assert!(
        matches!(source.pcode[0],PcodeOp::IntOr{lhs,..} if lhs.space==SpaceKind::Constant && lhs.offset==0)
    );
}

#[test]
fn reserved_encodings_and_adjacent_instructions_still_stop() {
    for word in [0x2a018000, 0x32400000, 0xb240fc00, 0x3200fc00] {
        assert!(
            matches!(decode(word).pcode[0], PcodeOp::Unsupported { .. }),
            "{word:x}"
        );
    }
    assert_eq!(evaluate(1, 2, 32, LogicalShift::Ror, 32), None);
    assert_eq!(immediate_mask(32, 1, 0, 0), None);
}

#[test]
fn orr_writes_zero_extended_result_and_preserves_latched_nzcv() {
    use ghidra_hopper_rs::arm64_flags::arithmetic;
    let before = arithmetic(0x80000000, 1, 32, true).unwrap().1;
    let mut flags = Some(before);
    let mut regs = [0u64; 32];
    regs[0] = u64::MAX;
    regs[1] = 0x80000000;
    for op in decode(0x2a010000).pcode {
        match op {
            PcodeOp::IntOr {
                dst,
                lhs,
                rhs,
                shift,
                amount,
            } => {
                let value = evaluate(
                    regs[lhs.offset as usize],
                    regs[rhs.offset as usize],
                    dst.size * 8,
                    shift,
                    amount,
                )
                .unwrap();
                regs[dst.offset as usize] = value;
            }
            PcodeOp::ClobberNzcv => flags = None,
            PcodeOp::FlagArithmetic { .. } => panic!("ORR must not set flags"),
            _ => panic!("unexpected ORR lowering"),
        }
    }
    assert_eq!(regs[0], u32::MAX as u64);
    assert_eq!(flags, Some(before));
}

#[test]
fn writing_orr_clears_stale_string_argument_in_analyzer() {
    let words = [0x90000002u32, 0x91010042, 0xaa030042, 0x94000004];
    let mut bytes = vec![0; 128];
    for (i, word) in words.into_iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes[64..68].copy_from_slice(b"old\0");
    let mut prog = GhidraProgram::new("test", Segment::new("text", 0x1000, 0, bytes));
    prog.objc_stub_selectors.insert(0x101c, "go:".into());
    let mut analyzer = HopperHybridObjcAnalyzerRs::new();
    analyzer.analyze_function_zero_ipc(&mut prog, "test", 0x1000, 4, &[]);
    assert_eq!(analyzer.discovered_messages[0].parameters, vec!["x2"]);
}
