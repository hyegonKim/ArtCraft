use ghidra_hopper_rs::{
    arm64_atomic::add_value,
    arm64_indexed::{offset_address, IndexExtend},
    arm64_simd::expanded_bytes,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
fn decode(w: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        w,
        &Segment::new("test", 0x1000, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}

#[test]
fn vector_bitmasks_cover_every_pattern_register_and_scalar_upper_clear() {
    for imm in 0..=255u32 {
        let bytes: Vec<u8> = (0..8)
            .map(|n| if imm / (1 << n) % 2 == 1 { 255 } else { 0 })
            .collect();
        let expected = u64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(expanded_bytes(imm as u8), expected);
        for q in [0, 1] {
            for r in 0..32 {
                let word = 0x2f00e400 | (q << 30) | ((imm >> 5) << 16) | ((imm & 31) << 5) | r;
                let i = decode(word);
                assert!(
                    matches!(i.pcode[0],PcodeOp::VectorImmediate{register,low,high} if register==r as u8&&low==expected&&high==if q==1{expected}else{0})
                );
                assert!(i.pcode[0].value_destination().is_none());
            }
        }
    }
    for w in [0x4f00f400, 0x6f00f400, 0x6f00ec00] {
        assert!(matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }));
    }
}

#[test]
fn vector_pair_offsets_modes_and_banks_do_not_confuse_q31_with_sp() {
    for mode in [1, 2, 3] {
        for load in [false, true] {
            for imm in -64i32..64 {
                let w = 0xac000000
                    | (mode << 23)
                    | ((load as u32) << 22)
                    | (((imm as u32) & 127) << 15)
                    | (31 << 10)
                    | (31 << 5);
                let i = decode(w);
                assert_eq!(i.mnemonic, if load { "ldp" } else { "stp" });
                assert!(
                    matches!(i.pcode[0],PcodeOp::IntAdd{lhs,rhs,..} if lhs.offset==31&&rhs.offset==if mode==1{0}else{(imm as i64*16)as u64})
                );
                assert_eq!(
                    i.pcode
                        .iter()
                        .filter(|op| matches!(
                            op,
                            PcodeOp::VectorLoad { .. } | PcodeOp::VectorStore { .. }
                        ))
                        .count(),
                    2
                );
                assert!(i.pcode.iter().any(|op| matches!(
                    op,
                    PcodeOp::VectorLoad { register: 31, .. }
                        | PcodeOp::VectorStore { register: 31, .. }
                )));
                assert_eq!(i.pcode.len(), if mode == 2 { 4 } else { 5 });
            }
        }
    }
    assert!(matches!(
        decode(0xad400000).pcode[0],
        PcodeOp::Unsupported { .. }
    )); // duplicate load destination
    assert!(!matches!(
        decode(0xad000000).pcode[0],
        PcodeOp::Unsupported { .. }
    )); // duplicate store is defined
    for w in [0xac000400, 0x6c000400, 0x2c000400] {
        assert!(matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }));
    }
}

#[test]
fn atomic_aliases_return_old_values_and_preserve_ordering_in_ir() {
    for width in [32, 64] {
        for a in [0, 1, u64::MAX, 1 << 31, 1 << 63] {
            for b in [0, 1, u64::MAX, 0x8123456789abcdef] {
                let mask = if width == 32 {
                    u32::MAX as u64
                } else {
                    u64::MAX
                };
                assert_eq!(
                    add_value(a, b, width),
                    Some((a & mask, ((a as u128 + b as u128) & mask as u128) as u64))
                );
            }
        }
    }
    for size in [2, 3] {
        for a in [0, 1] {
            for r in [0, 1] {
                for rs in [0, 31] {
                    let w =
                        (size << 30) | 0x38200000 | (a << 23) | (r << 22) | (rs << 16) | (1 << 5);
                    assert!(
                        matches!(decode(w).pcode[0],PcodeOp::AtomicAdd{dst,src,addr,acquire,release} if dst.offset==0&&addr.offset==1&&dst.size==if size==2{4}else{8}&&acquire==(a==1)&&release==(r==1)&&src.space==if rs==31{SpaceKind::Constant}else{SpaceKind::Register})
                    );
                }
            }
        }
    }
    assert!(
        matches!(decode(0x885ffc20).pcode[0],PcodeOp::ExclusiveLoadAcquire{dst,addr} if dst.offset==0&&addr.offset==1)
    );
    assert!(
        matches!(decode(0x880ffc31).pcode[0],PcodeOp::ExclusiveStoreRelease{status,src,addr} if status.offset==15&&status.size==4&&src.offset==17&&addr.offset==1)
    );
    for imm in 0..16 {
        assert!(matches!(
            decode(0xd503305f | (imm << 8)).pcode[0],
            PcodeOp::ClearExclusive
        ));
    }
    for w in [
        0x38200020, 0x78200020, 0xb8e01020, 0xb8e0003f, 0x885f7c20, 0x885ffc3f, 0x8800fc20,
        0x8801fc20, 0x881ffc20,
    ] {
        assert!(
            matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }),
            "{w:x}"
        );
    }
}

#[test]
fn indexed_integer_transfers_extend_before_scaling_and_capture_addresses() {
    for extend in [
        IndexExtend::Uxtw,
        IndexExtend::Uxtx,
        IndexExtend::Sxtw,
        IndexExtend::Sxtx,
    ] {
        for shift in 0..=3 {
            for index in [0, 1, u64::MAX, 0xdeadbeeffffffffe] {
                let signed: i128 = match extend {
                    IndexExtend::Uxtw => (index & 0xffffffff) as i128,
                    IndexExtend::Uxtx => index as i128,
                    IndexExtend::Sxtw => {
                        if index & 0x80000000 != 0 {
                            (index & 0xffffffff) as i128 - (1 << 32)
                        } else {
                            (index & 0xffffffff) as i128
                        }
                    }
                    IndexExtend::Sxtx => {
                        if index >= 1 << 63 {
                            index as i128 - (1i128 << 64)
                        } else {
                            index as i128
                        }
                    }
                };
                assert_eq!(
                    offset_address(0x1000, index, extend, shift),
                    Some(((0x1000i128 + signed * (1 << shift)) & ((1i128 << 64) - 1)) as u64)
                );
            }
        }
    }
    for size in 0..4 {
        for opc in [0, 1] {
            for option in [2, 3, 6, 7] {
                for scaled in [0, 1] {
                    let w = 0x38200800
                        | (size << 30)
                        | (opc << 22)
                        | (option << 13)
                        | (scaled << 12)
                        | (31 << 16);
                    let i = decode(w);
                    assert!(
                        matches!(i.pcode[0],PcodeOp::RegisterOffset{base,index,shift,..} if base.offset==0&&index.space==SpaceKind::Constant&&index.offset==0&&shift==if scaled==1{size as u8}else{0})
                    );
                    assert!(i
                        .pcode
                        .iter()
                        .any(|op| matches!(op, PcodeOp::Load { .. } | PcodeOp::Store { .. })));
                }
            }
        }
    }
    assert!(decode(0xb8a17800)
        .pcode
        .iter()
        .any(|op| matches!(op, PcodeOp::SignExtend { .. })));
    for w in [0x3c210800, 0xf8a16800, 0x38a16800, 0xf8610800] {
        assert!(matches!(decode(w).pcode[0], PcodeOp::Unsupported { .. }));
    }
}

#[test]
fn calls_leave_monitor_and_simd_state_unknown_not_presumed_preserved() {
    for w in [0x94000000, 0xd63f0100] {
        let i = decode(w);
        assert!(i
            .pcode
            .iter()
            .any(|op| matches!(op, PcodeOp::ClobberExclusive)));
        assert!(i.pcode.iter().any(|op| matches!(op, PcodeOp::ClobberSimd)));
        assert!(matches!(i.pcode.last(), Some(PcodeOp::ClobberNzcv)));
    }
}
