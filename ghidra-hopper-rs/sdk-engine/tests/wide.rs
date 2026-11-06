use ghidra_hopper_rs::{
    arm64_integer::{extended, logical_flags, multiply_add},
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
fn decode(w: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        w,
        &Segment::new("text", 0x1000, 0, vec![0; 32]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}
#[test]
fn logical_flags_and_register_31_roles_are_explicit() {
    assert!(matches!(
        decode(0x72001f7f).pcode[0],
        PcodeOp::Logical {
            dst: None,
            set_flags: true,
            width: 32,
            ..
        }
    ));
    assert!(
        matches!(decode(0x92401c1f).pcode[0],PcodeOp::Logical{dst:Some(dst),set_flags:false,..} if dst.space==SpaceKind::Register && dst.offset==31)
    );
    assert!(
        matches!(decode(0x8a1f03e0).pcode[0],PcodeOp::Logical{lhs,rhs,..} if lhs.space==SpaceKind::Constant && rhs.space==SpaceKind::Constant)
    );
    for width in [32, 64] {
        for value in [0, 1, 1u64 << (width - 1), u64::MAX] {
            let f = logical_flags(value, width).unwrap();
            assert!(!f.c && !f.v);
            assert_eq!(
                f.z,
                value
                    & (if width == 32 {
                        u32::MAX as u64
                    } else {
                        u64::MAX
                    })
                    == 0
            );
        }
    }
}
#[test]
fn extension_and_multiply_width_boundaries_wrap_without_signed_host_overflow() {
    assert_eq!(extended(0xff, 64, 4, 4), Some(u64::MAX - 15));
    assert_eq!(
        extended(0x1234567880000000, 64, 6, 0),
        Some(0xffffffff80000000)
    );
    assert_eq!(extended(0xffffffff00000001, 64, 2, 4), Some(16));
    assert_eq!(extended(1, 64, 0, 5), None);
    assert_eq!(extended(1, 16, 0, 0), None);
    assert_eq!(multiply_add(u64::MAX, u64::MAX, 1, 64, false), Some(2));
    assert_eq!(multiply_add(u64::MAX, 2, 0, 32, true), Some(2));
    assert!(
        matches!(decode(0x8b21401f).pcode[0],PcodeOp::RegisterArithmetic{dst:Some(dst),..}if dst.offset==31)
    );
    assert!(matches!(
        decode(0xeb21401f).pcode[0],
        PcodeOp::RegisterArithmetic {
            dst: None,
            set_flags: true,
            ..
        }
    ));
    assert!(matches!(
        decode(0xcb01001f).pcode[0],
        PcodeOp::RegisterArithmetic {
            dst: None,
            set_flags: false,
            ..
        }
    ));
}
#[test]
fn reserved_encodings_and_different_instruction_families_stay_unsupported() {
    for w in [
        0x0a018000, 0x12400000, 0xf240fc00, 0x9a810800, 0xba810400, 0x8b211400, 0xcbc10000,
        0x1b210000, 0xfa400cd4, 0x69400400, 0xfc800000, 0x7dc00000,
    ] {
        assert!(
            matches!(decode(w).pcode.first(), Some(PcodeOp::Unsupported { .. })),
            "{w:x}"
        );
    }
}
#[test]
fn simd_single_widths_clear_upper_bits_and_pair_widths_are_not_gpr_aliases() {
    for (w, size) in [
        (0x3d400000, 1),
        (0x7d400000, 2),
        (0xbd400000, 4),
        (0xfd400000, 8),
    ] {
        let i = decode(w);
        assert!(i
            .pcode
            .iter()
            .any(|op| matches!(op,PcodeOp::VectorLoadSized{register:0,size:s,..}if *s==size)));
        assert!(i.pcode.iter().all(|op| op
            .value_destination()
            .is_none_or(|v| v.space != SpaceKind::Register)));
    }
    for (w, size) in [(0x2d400400, 4), (0x6d400400, 8)] {
        assert_eq!(
            decode(w)
                .pcode
                .iter()
                .filter(|op| matches!(op,PcodeOp::VectorLoadSized{size:s,..}if *s==size))
                .count(),
            2
        );
    }
    assert!(matches!(
        decode(0x2d400000).pcode[0],
        PcodeOp::Unsupported { .. }
    ));
    assert!(!matches!(
        decode(0x6dc00400).pcode[0],
        PcodeOp::Unsupported { .. }
    )); // d0,d1 base x0 numeric overlap is defined.
    assert!(decode(0x3cc103ff)
        .pcode
        .iter()
        .any(|op| matches!(op, PcodeOp::VectorLoad { register: 31, .. }))); // ldur q31,[sp,#16]
}
#[test]
fn all_new_scalar_destinations_invalidate_stale_string_inference() {
    for word in [0x8a000042, 0x9a810402, 0x8b214002, 0xcb000042, 0x9b011002] {
        let mut bytes = vec![0; 256];
        bytes[128..132].copy_from_slice(b"old\0");
        for (n, w) in [0x90000002u32, 0x91020042, word, 0x94000009, 0xd65f03c0]
            .iter()
            .enumerate()
        {
            bytes[n * 4..n * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut p = GhidraProgram::new("stale", Segment::new("text", 0x1000, 0, bytes));
        p.objc_stub_selectors.insert(0x1030, "go:".into());
        let mut a = HopperHybridObjcAnalyzerRs::new();
        a.analyze_function_zero_ipc(&mut p, "stale", 0x1000, 16, &[]);
        assert_eq!(a.discovered_messages[0].parameters, vec!["x2"], "{word:x}");
    }
}
