use ghidra_hopper_rs::{
    arm64_flags::{Condition, Nzcv},
    arm64_indexed::{address, offset_address, IndexExtend},
    arm64_integer::{conditional_compare, signed_multiply_high},
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
#[test]
fn conditional_compare_decodes_real_words_and_has_no_gpr_destination() {
    for word in [0xfa4008c4, 0xfa401924, 0x7a4129e0, 0x7a412aa0] {
        let i = decode(word);
        assert_eq!(i.mnemonic, "ccmp");
        assert!(matches!(
            i.pcode[0],
            PcodeOp::ConditionalCompare { subtract: true, .. }
        ));
        assert_eq!(i.pcode[0].value_destination(), None);
        assert!(!i.is_call && !i.is_ret && !i.is_conditional_jump);
    }
    let register = decode(0xfa5f03e5); // xzr, xzr
    assert!(
        matches!(register.pcode[0],PcodeOp::ConditionalCompare{lhs,rhs,..}if lhs.space==SpaceKind::Constant && rhs.space==SpaceKind::Constant && rhs.offset==0)
    );
    let immediate = decode(0xfa5f0be5); // xzr, #31
    assert!(
        matches!(immediate.pcode[0],PcodeOp::ConditionalCompare{lhs,rhs,nzcv:5,..}if lhs.space==SpaceKind::Constant && rhs.space==SpaceKind::Constant && rhs.offset==31)
    );
}
#[test]
fn conditional_compare_reads_incoming_flags_and_false_path_sets_every_fallback_bit() {
    let eq = Condition::from_code(0).unwrap();
    for width in [32, 64] {
        for fallback in 0..16 {
            assert_eq!(
                conditional_compare(7, 7, width, eq, fallback, true, Nzcv::from_bits(0))
                    .unwrap()
                    .bits(),
                fallback
            );
            assert_eq!(
                conditional_compare(7, 7, width, eq, fallback, true, Nzcv::from_bits(4))
                    .unwrap()
                    .bits(),
                6
            );
            assert_eq!(
                conditional_compare(u64::MAX, 1, width, eq, fallback, false, Nzcv::from_bits(4))
                    .unwrap()
                    .bits(),
                6
            ); // zero + carry.
        }
        for condition in [14, 15] {
            assert_eq!(
                conditional_compare(
                    7,
                    7,
                    width,
                    Condition::from_code(condition).unwrap(),
                    15,
                    true,
                    Nzcv::from_bits(0)
                )
                .unwrap()
                .bits(),
                6
            );
        }
    }
    assert!(conditional_compare(0, 0, 16, eq, 0, true, Nzcv::from_bits(0)).is_none());
    assert!(conditional_compare(0, 0, 64, eq, 16, true, Nzcv::from_bits(0)).is_none());
}
#[test]
fn smulh_uses_signed_high_half_and_zr_not_sp() {
    for (a, b, wanted) in [
        (u64::MAX, 1, u64::MAX),
        (u64::MAX, u64::MAX, 0),
        (1 << 63, 1 << 63, 1 << 62),
        ((1u64 << 63) - 1, (1u64 << 63) - 1, (1u64 << 62) - 1),
        (1 << 63, u64::MAX, 0),
        (0, u64::MAX, 0),
    ] {
        assert_eq!(signed_multiply_high(a, b), wanted);
    }
    assert!(
        matches!(decode(0x9b4a7d29).pcode[0],PcodeOp::SignedMultiplyHigh{dst:Some(dst),lhs,rhs}if dst.offset==9 && lhs.offset==9 && rhs.offset==10)
    );
    assert!(
        matches!(decode(0x9b5f7fff).pcode[0],PcodeOp::SignedMultiplyHigh{dst:None,lhs,rhs}if lhs.space==SpaceKind::Constant && rhs.space==SpaceKind::Constant)
    );
}
fn memory_word(size: u8, load: bool, option: u32, scaled: bool, rn: u32, rm: u32, vt: u32) -> u32 {
    0x3c200800
        | (if size == 16 { 0 } else { size.trailing_zeros() } << 30)
        | ((if size == 16 { 2 } else { 0 }) | u32::from(load)) << 22
        | option << 13
        | u32::from(scaled) << 12
        | rn << 5
        | rm << 16
        | vt
}
#[test]
fn simd_register_offsets_cover_all_widths_extensions_and_bank_roles_without_writeback() {
    for size in [1, 2, 4, 8, 16] {
        for option in [2, 3, 6, 7] {
            for scaled in [false, true] {
                for load in [false, true] {
                    let i = decode(memory_word(size, load, option, scaled, 31, 31, 31));
                    assert_eq!(i.pcode.len(), 2);
                    assert!(
                        matches!(i.pcode[0],PcodeOp::RegisterOffset{base,index,shift,..}if base.space==SpaceKind::Register && base.offset==31 && index.space==SpaceKind::Constant && index.offset==0 && shift==if scaled{size.trailing_zeros() as u8}else{0})
                    );
                    assert!(i.pcode.iter().all(|op| op
                        .value_destination()
                        .is_none_or(|d| d.space != SpaceKind::Register)));
                    match &i.pcode[1] {
                        PcodeOp::VectorLoad { register, .. }
                        | PcodeOp::VectorStore { register, .. } => {
                            assert_eq!(size, 16);
                            assert_eq!(*register, 31);
                        }
                        PcodeOp::VectorLoadSized {
                            register, size: s, ..
                        }
                        | PcodeOp::VectorStoreSized {
                            register, size: s, ..
                        } => {
                            assert_eq!(*s, size);
                            assert_eq!(*register, 31);
                        }
                        _ => panic!("wrong transfer"),
                    }
                }
            }
        }
    }
    let actual = decode(0x3ca96ac0);
    assert_eq!(actual.mnemonic, "str"); // Correct the previous report's "load" classification.
    assert!(
        matches!(actual.pcode[0],PcodeOp::RegisterOffset{base,index,shift:0,..}if base.offset==22 && index.offset==9)
    );
    assert!(matches!(
        actual.pcode[1],
        PcodeOp::VectorStore { register: 0, .. }
    ));
    assert_eq!(
        offset_address(0x1000, 0xffffffff, IndexExtend::Sxtw, 4),
        Some(0xff0)
    );
    assert_eq!(
        offset_address(0x1000, 0xdeadbeef00000002, IndexExtend::Uxtw, 4),
        Some(0x1020)
    );
    assert_eq!(offset_address(0, 0, IndexExtend::Uxtx, 5), None);
    assert_eq!(address(0, 0, IndexExtend::Uxtx, 4), None); // legacy halfword helper bound.
}
#[test]
fn reserved_and_unrequested_encodings_remain_explicitly_unsupported() {
    for word in [
        0xfa4008d4, 0xfa400cc4, 0xda4008c4, 0x1b417c00, 0x9bc17c00, 0x9b417800, 0x7ce16800,
    ] {
        assert!(
            matches!(
                decode(word).pcode.first(),
                Some(PcodeOp::Unsupported { .. })
            ),
            "{word:x}"
        );
    }
    for option in [0, 1, 4, 5] {
        assert!(matches!(
            decode(memory_word(16, true, option, true, 0, 1, 0)).pcode[0],
            PcodeOp::Unsupported { .. }
        ));
    }
}
#[test]
fn ccmp_preserves_argument_provenance_while_smulh_invalidates_old_strings() {
    for (word, wanted) in [(0xfa410040, "\"old\""), (0x9b417c42, "x2")] {
        let mut data = vec![0; 256];
        data[128..132].copy_from_slice(b"old\0");
        for (n, w) in [0x90000002u32, 0x91020042, word, 0x94000009, 0xd65f03c0]
            .iter()
            .enumerate()
        {
            data[n * 4..n * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut p = GhidraProgram::new("flags", Segment::new("text", 0x1000, 0, data));
        p.objc_stub_selectors.insert(0x1030, "take:".into());
        let mut a = HopperHybridObjcAnalyzerRs::new();
        a.analyze_function_zero_ipc(&mut p, "flags", 0x1000, 16, &[]);
        assert_eq!(a.discovered_messages[0].parameters, vec![wanted]);
    }
}
#[test]
fn ccmp_then_conditional_branch_keeps_both_paths_and_respects_budget() {
    let words = [
        0xfa410000u32,
        0x5400006c,
        0xd2800440,
        0xd65f03c0,
        0xd2800220,
        0xd65f03c0,
    ];
    for budget in [1, 6] {
        let mut s = Segment::new(
            "text",
            0x1000,
            0,
            words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        );
        let f = Procedure::analyze_arm64(
            "ccmp",
            0x1000,
            budget,
            &mut s,
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        assert_eq!(f.is_incomplete(), budget == 1);
        assert!(f.analysis_work <= budget);
        let text = f.render_four_modes(&s);
        assert!(text.contains("condition reads incoming NZCV"));
        if budget == 6 {
            assert!(text.contains("loc_1010"));
            assert!(text.contains("loc_1008"));
        }
    }
}
