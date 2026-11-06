use ghidra_hopper_rs::{
    arm64_data::{movk_value, select_value, sign_extend_word},
    arm64_flags::{Condition, Nzcv},
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
fn decode(w: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        w,
        &Segment::new("test", 0x1000, 0, vec![0; 128]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}

#[test]
fn wide_moves_preserve_halfwords_width_and_zero_register_semantics() {
    for width in [32u8, 64] {
        for shift in (0..width).step_by(16) {
            for imm in 0..=u16::MAX {
                let old = 0x8123456789abcdef;
                let mut expected = 0;
                for bit in 0..width {
                    let set = if (shift..shift + 16).contains(&bit) {
                        (imm as u64) & (1 << (bit - shift)) != 0
                    } else {
                        old & (1 << bit) != 0
                    };
                    if set {
                        expected |= 1u64 << bit;
                    }
                }
                assert_eq!(movk_value(old, imm, shift, width), Some(expected));
            }
        }
    }
    assert_eq!(movk_value(0, 1, 32, 32), None);
    assert_eq!(movk_value(0, 1, 3, 64), None);
    for sf in [0, 1] {
        for hw in 0..4 {
            for opc in [0, 2, 3] {
                for rd in [0, 31] {
                    let w = (sf << 31) | (opc << 29) | 0x12800000 | (hw << 21) | (0x6c69 << 5) | rd;
                    let i = decode(w);
                    if sf == 0 && hw >= 2 {
                        assert!(matches!(i.pcode[0], PcodeOp::Unsupported { .. }));
                        continue;
                    }
                    let dst = i.pcode[0].value_destination().unwrap();
                    assert_eq!(
                        dst.space,
                        if rd == 31 {
                            SpaceKind::Unique
                        } else {
                            SpaceKind::Register
                        }
                    );
                    assert_eq!(dst.size, if sf == 1 { 8 } else { 4 });
                    if opc == 0 {
                        let PcodeOp::Copy { src, .. } = i.pcode[0] else {
                            panic!()
                        };
                        let mask = if sf == 1 { u64::MAX } else { u32::MAX as u64 };
                        assert_eq!(src.offset, (!(0x6c69u64 << (hw * 16))) & mask);
                    }
                }
            }
        }
    }
    assert!(matches!(
        decode(0x32800000).pcode[0],
        PcodeOp::Unsupported { .. }
    ));
}

#[test]
fn csel_matches_conditions_all_flags_widths_and_zero_inputs() {
    for width in [32, 64] {
        for code in 0..16 {
            for flags in 0..16 {
                let n = flags & 8 != 0;
                let z = flags & 4 != 0;
                let c = flags & 2 != 0;
                let v = flags & 1 != 0;
                let expected = [
                    z,
                    !z,
                    c,
                    !c,
                    n,
                    !n,
                    v,
                    !v,
                    c && !z,
                    !c || z,
                    n == v,
                    n != v,
                    !z && n == v,
                    z || n != v,
                    true,
                    true,
                ][code as usize];
                let a = 0x8123456789abcdef;
                let b = 0xfedcba9876543210;
                assert_eq!(
                    select_value(
                        a,
                        b,
                        width,
                        Condition::from_code(code).unwrap(),
                        Nzcv::from_bits(flags)
                    ),
                    Some(
                        (if expected { a } else { b })
                            & if width == 32 {
                                u32::MAX as u64
                            } else {
                                u64::MAX
                            }
                    )
                );
                let w = 0x1a800000
                    | if width == 64 { 1 << 31 } else { 0 }
                    | (31 << 16)
                    | ((code as u32) << 12)
                    | (31 << 5)
                    | 31;
                assert!(
                    matches!(decode(w).pcode[0],PcodeOp::ConditionalSelect{dst,if_true,if_false,..} if dst.space==SpaceKind::Unique && if_true.space==SpaceKind::Constant && if_false.space==SpaceKind::Constant)
                );
            }
        }
    }
    for w in [0xba810400, 0xfa810000, 0x9a810800] {
        assert!(matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }));
    }
}

#[test]
fn signed_loads_snapshot_addresses_and_reject_unpredictable_writeback() {
    for value in [0, 1, 0x7fffffff, 0x80000000, 0xffffffff, 0xdeadbeef80000001] {
        let low = value & 0xffffffff;
        let expected = if low >= 0x80000000 {
            u64::MAX - (0xffffffff - low)
        } else {
            low
        };
        assert_eq!(sign_extend_word(value), expected);
    }
    for w in [0xb9800000, 0xb98023ff, 0xb89fc001, 0xb8804401, 0xb8804c01] {
        let i = decode(w);
        assert!(i
            .pcode
            .iter()
            .any(|p| matches!(p,PcodeOp::Load{dst,..} if dst.size==4)));
        assert!(i
            .pcode
            .iter()
            .any(|p| matches!(p,PcodeOp::SignExtend{dst,src} if dst.size==8 && src.size==4)));
        assert!(matches!(i.pcode[0],PcodeOp::IntAdd{dst,..} if dst.space==SpaceKind::Unique));
    }
    for w in [0xb8804400, 0xb8804c00, 0xb8804801, 0x39800000, 0xbd800000] {
        assert!(
            matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }),
            "{w:x}"
        );
    }
}

#[test]
fn acquire_and_thread_reads_are_explicit_and_never_constant_or_plain_loads() {
    for w in [0x88dffc00, 0xc8dffc20, 0xc8dfffe0] {
        let i = decode(w);
        assert!(matches!(i.pcode[0], PcodeOp::LoadAcquire { .. }));
        assert_eq!(i.pcode.len(), 1);
    }
    assert!(matches!(decode(0xc8dfffe0).pcode[0],PcodeOp::LoadAcquire{addr,..} if addr.offset==31));
    for w in [0x885f7c00, 0xb8e01020, 0x88dffc1f, 0xd53bd060] {
        assert!(matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }));
    }
    assert!(
        matches!(decode(0xd53bd056).pcode[0],PcodeOp::ReadThreadPointer{dst} if dst.offset==22)
    );
    assert!(
        matches!(decode(0xd53bd05f).pcode[0],PcodeOp::ReadThreadPointer{dst} if dst.space==SpaceKind::Unique)
    );
}

#[test]
fn tagged_addg_subg_are_not_misdecoded_as_plain_arithmetic() {
    for w in [0x91800020, 0xd1800020, 0x11800020, 0x51800020] {
        assert!(
            matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }),
            "{w:x}"
        );
    }
    assert!(matches!(
        decode(0x91000020).pcode[0],
        PcodeOp::IntAdd { .. }
    ));
}

#[test]
fn value_writes_invalidate_old_objc_strings_including_preexisting_sub() {
    for w in [
        0xd1000442, 0xf2ad8d22, 0x9a800002, 0xd53bd042, 0xc8dffc02, 0xb9800002,
    ] {
        let mut bytes = vec![0; 128];
        for (n, word) in [0x90000002u32, 0x91010042, w, 0x94000004]
            .iter()
            .enumerate()
        {
            bytes[n * 4..n * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes[64..68].copy_from_slice(b"old\0");
        let mut p = GhidraProgram::new("cache", Segment::new("text", 0x1000, 0, bytes));
        p.objc_stub_selectors.insert(0x101c, "go:".into());
        let mut a = HopperHybridObjcAnalyzerRs::new();
        a.analyze_function_zero_ipc(&mut p, "cache", 0x1000, 4, &[]);
        assert_eq!(a.discovered_messages[0].parameters, vec!["x2"], "{w:x}");
    }
}

#[test]
fn declared_boundaries_do_not_become_invented_fallthroughs_or_tail_calls() {
    use ghidra_hopper_rs::{cfg_tags::Procedure, segment::Section};
    for section_boundary in [false, true] {
        let mut seg = Segment::new(
            "text",
            0x1000,
            0,
            [0x52800020u32, 0xd65f03c0]
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect(),
        );
        if section_boundary {
            seg.sections = vec![
                Section {
                    name: "code".into(),
                    start_address: 0x1000,
                    length: 4,
                    flags: 0x80000400,
                },
                Section {
                    name: "stub".into(),
                    start_address: 0x1004,
                    length: 4,
                    flags: 0x80000408,
                },
            ];
        } else {
            seg.function_starts.insert(0x1004);
        }
        let p = Procedure::analyze_arm64(
            "edge",
            0x1000,
            16,
            &mut seg,
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        assert!(p.is_incomplete());
        assert_eq!(p.basic_blocks[0].instructions.len(), 1);
        assert!(p.basic_blocks[0].successors.is_empty());
        assert!(p
            .render_four_modes(&seg)
            .contains("continuation unverified"));
        seg.data[..4].copy_from_slice(&0x14000001u32.to_le_bytes()); // explicit B across boundary is real flow
        let p = Procedure::analyze_arm64(
            "tail",
            0x1000,
            16,
            &mut seg,
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        assert!(!p.is_incomplete());
        assert_eq!(
            p.basic_blocks
                .iter()
                .map(|b| b.instructions.len())
                .sum::<usize>(),
            2
        );
    }
}
