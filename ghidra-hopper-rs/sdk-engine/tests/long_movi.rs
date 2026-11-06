use ghidra_hopper_rs::{
    arm64_integer::signed_multiply_add_long,
    arm64_simd::movi_immediate,
    cfg_tags::Procedure,
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;

fn decode(word: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        word,
        &Segment::new("text", 0x1000, 0, vec![0; 32]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}
fn long(rd: u32, rn: u32, rm: u32, ra: u32) -> u32 {
    0x9b200000 | rd | rn << 5 | ra << 10 | rm << 16
}
fn movi(q: u32, op: u32, cmode: u32, imm: u32, rd: u32) -> u32 {
    0x0f000400 | q << 30 | op << 29 | cmode << 12 | (imm >> 5) << 16 | (imm & 31) << 5 | rd
}

#[test]
fn long_multiply_real_words_use_w_sources_x_addend_and_smull_alias() {
    for word in [
        0x9b230962, 0x9b251164, 0x9b276d84, 0x9b247d86, 0x9b356226, 0x9b267e27, 0x9b264dc4,
        0x9b247dc5,
    ] {
        let i = decode(word);
        let ra = (word >> 10) & 31;
        assert_eq!(i.mnemonic, if ra == 31 { "smull" } else { "smaddl" });
        assert_eq!(i.raw_args.len(), if ra == 31 { 3 } else { 4 });
        assert!(
            matches!(i.pcode[0], PcodeOp::SignedMultiplyAddLong { dst: Some(dst), lhs, rhs, addend }
            if dst.size == 8 && lhs.size == 4 && rhs.size == 4 && addend.size == 8)
        );
        assert!(!i.is_call && !i.is_ret && !i.is_conditional_jump);
    }
    let i = decode(long(31, 31, 31, 31));
    assert!(
        matches!(i.pcode[0], PcodeOp::SignedMultiplyAddLong { dst: None, lhs, rhs, addend }
        if lhs.space == SpaceKind::Constant && rhs.space == SpaceKind::Constant && addend.space == SpaceKind::Constant)
    );
    assert!(i.raw_args.iter().all(|arg| !arg.contains("sp")));
    assert_eq!(
        decode(long(2, 2, 1, 2)).pcode[0]
            .value_destination()
            .unwrap()
            .offset,
        2
    );
}

#[test]
fn signed_w_products_ignore_high_bits_and_accumulate_modulo_64() {
    for (lhs, rhs, addend, result) in [
        (0xffffffff, 2, 7, 5),
        (0x80000000, 2, 0, 0xffffffff00000000),
        (0x80000000, 0x80000000, 0, 1 << 62),
        (0x7fffffff, 0x7fffffff, 0, 0x3fffffff00000001),
        (0xffffffff, 0xffffffff, u64::MAX, 0),
        (0x80000000, 1, 1 << 63, 0x7fffffff80000000),
    ] {
        assert_eq!(signed_multiply_add_long(lhs, rhs, addend), result);
        assert_eq!(
            signed_multiply_add_long(lhs | 0xabcddcba00000000, rhs | 0x1234567800000000, addend),
            result
        );
    }
}

#[test]
fn movi_replicates_each_lane_and_clears_upper_half_for_every_integer_form() {
    for cmode in [0, 2, 4, 6, 8, 10, 12, 13, 14] {
        for imm in 0..=255u32 {
            // Construct expected bytes, independently of the shift/replicate helper.
            let mut lane = vec![
                0u8;
                if cmode == 14 {
                    1
                } else if (8..=10).contains(&cmode) {
                    2
                } else {
                    4
                }
            ];
            let position = match cmode {
                0 | 2 | 4 | 6 => cmode / 2,
                8 | 10 => (cmode - 8) / 2,
                12 | 13 => cmode - 11,
                _ => 0,
            };
            lane[position as usize] = imm as u8;
            if cmode == 12 || cmode == 13 {
                lane[..position as usize].fill(255);
            }
            let bytes: Vec<u8> = lane.iter().copied().cycle().take(8).collect();
            let expected = u64::from_le_bytes(bytes.try_into().unwrap());
            assert_eq!(
                movi_immediate(false, cmode as u8, imm as u8),
                Some(expected)
            );
            for q in [0, 1] {
                for rd in 0..32 {
                    let i = decode(movi(q, 0, cmode, imm, rd));
                    assert!(
                        matches!(i.pcode[0], PcodeOp::VectorImmediate { register, low, high }
                        if register == rd as u8 && low == expected && high == if q == 1 { expected } else { 0 })
                    );
                    assert_eq!(i.pcode[0].value_destination(), None);
                }
            }
        }
    }
    assert_eq!(decode(0x0f000600).raw_args, ["v0.2s", "#0x10"]);
    assert_eq!(
        decode(movi(1, 0, 13, 0x12, 31)).raw_args,
        ["v31.4s", "#0x12", "msl #16"]
    );
}

#[test]
fn adjacent_families_and_reserved_bits_remain_unsupported() {
    for word in [0x1b210000, 0x9b218000, 0x9ba10000, 0x9ba17c00] {
        assert!(
            matches!(decode(word).pcode[0], PcodeOp::Unsupported { .. }),
            "{word:x}"
        );
    }
    for op in [0, 1] {
        for cmode in 0..16 {
            let valid = if op == 1 {
                cmode == 14
            } else {
                matches!(cmode, 0 | 2 | 4 | 6 | 8 | 10 | 12 | 13 | 14)
            };
            assert_eq!(
                matches!(
                    decode(movi(1, op, cmode, 0x81, 0)).pcode[0],
                    PcodeOp::VectorImmediate { .. }
                ),
                valid
            );
        }
    }
    for bit in [11, 19, 31] {
        assert!(matches!(
            decode(movi(1, 0, 14, 0x81, 0) | 1 << bit).pcode[0],
            PcodeOp::Unsupported { .. }
        ));
    }
    assert_eq!(movi_immediate(false, 16, 0), None);
    assert_eq!(movi_immediate(true, 0, 0), None);
}

#[test]
fn long_destination_invalidates_strings_while_movi_preserves_gpr_bank() {
    for (word, wanted) in [
        (long(2, 2, 1, 2), "x2"),
        (movi(1, 0, 14, 0xff, 2), "\"old\""),
    ] {
        let mut data = vec![0; 256];
        data[128..132].copy_from_slice(b"old\0");
        for (n, w) in [0x90000002u32, 0x91020042, word, 0x94000009, 0xd65f03c0]
            .iter()
            .enumerate()
        {
            data[n * 4..n * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut p = GhidraProgram::new("long", Segment::new("text", 0x1000, 0, data));
        p.objc_stub_selectors.insert(0x1030, "take:".into());
        let mut a = HopperHybridObjcAnalyzerRs::new();
        a.analyze_function_zero_ipc(&mut p, "long", 0x1000, 16, &[]);
        assert_eq!(a.discovered_messages[0].parameters, vec![wanted]);
    }
}

#[test]
fn new_operations_reach_return_and_keep_instruction_work_bounded() {
    let words = [long(2, 2, 1, 2), movi(0, 0, 0, 0x10, 0), 0xd65f03c0];
    for budget in [1, 2, 3] {
        let mut s = Segment::new(
            "text",
            0x1000,
            0,
            words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        );
        let f = Procedure::analyze_arm64(
            "long",
            0x1000,
            budget,
            &mut s,
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        assert_eq!(f.is_incomplete(), budget < 3);
        assert_eq!(f.analysis_work, budget);
        let text = f.render_four_modes(&s);
        assert!(text.contains("signed 32-bit sources"));
        if budget == 3 {
            assert!(text.contains("u128_from_halves"));
        }
    }
}
