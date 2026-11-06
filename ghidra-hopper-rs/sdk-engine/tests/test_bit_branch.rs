use ghidra_hopper_rs::{
    arm64_test_branch::taken,
    cfg_tags::Procedure,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
fn decode_at(addr: u64, word: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        addr,
        word,
        &Segment::new("text", addr, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}
fn encode(bit: u8, nonzero: bool, rt: u8, imm14: u16) -> u32 {
    0x36000000
        | (((bit as u32) >> 5) << 31)
        | ((nonzero as u32) << 24)
        | (((bit as u32) & 31) << 19)
        | (((imm14 as u32) & 0x3fff) << 5)
        | (rt as u32)
}

#[test]
fn hopper_words_have_correct_bit_register_and_target() {
    let a = decode_at(0x100002fe8, 0x37000088);
    assert_eq!(a.mnemonic, "tbnz");
    assert_eq!(a.branch_target, Some(0x100002ff8));
    assert!(
        matches!(a.pcode[0],PcodeOp::TestBitBranch{value,bit:0,nonzero:true,..} if value.offset==8 && value.size==4)
    );
    let b = decode_at(0x100003054, 0x360000a8);
    assert_eq!(b.mnemonic, "tbz");
    assert_eq!(b.branch_target, Some(0x100003068));
    assert!(
        matches!(b.pcode[0],PcodeOp::TestBitBranch{value,bit:0,nonzero:false,..} if value.offset==8)
    );
}

#[test]
fn every_bit_register_and_polarity_decodes_without_sp_confusion() {
    for bit in 0..64 {
        for nonzero in [false, true] {
            for rt in 0..32 {
                let insn = decode_at(0x1000, encode(bit, nonzero, rt, 1));
                let expected_size = if bit < 32 { 4 } else { 8 };
                assert!(
                    insn.is_conditional_jump
                        && !insn.is_call
                        && !insn.is_ret
                        && !insn.is_unconditional_jump
                );
                match insn.pcode[0] {
                    PcodeOp::TestBitBranch {
                        value,
                        bit: b,
                        nonzero: n,
                        target,
                    } => {
                        assert_eq!(
                            (b, n, target, value.size),
                            (bit, nonzero, 0x1004, expected_size)
                        );
                        assert_eq!(
                            value.space,
                            if rt == 31 {
                                SpaceKind::Constant
                            } else {
                                SpaceKind::Register
                            }
                        );
                        assert_eq!(value.offset, if rt == 31 { 0 } else { rt as u64 });
                    }
                    _ => panic!("missing bit branch"),
                }
                assert_eq!(insn.pcode.len(), 1);
            }
        }
    }
}

#[test]
fn all_signed_offsets_and_pc_wraparound_are_correct() {
    for bits in 0..0x4000u16 {
        let signed = if bits < 0x2000 {
            bits as i64
        } else {
            bits as i64 - 0x4000
        };
        for pc in [0u64, 0x100000000, u64::MAX - 3] {
            assert_eq!(
                decode_at(pc, encode(63, true, 0, bits)).branch_target,
                Some(pc.wrapping_add((signed * 4) as u64))
            );
        }
    }
}

#[test]
fn bit_decisions_match_division_and_do_not_consult_flags() {
    for bit in 0..64 {
        for value in [0, u64::MAX, 1u64 << bit, !(1u64 << bit), 0x8123456789abcdef] {
            let set = (value / (1u64 << bit)) % 2 == 1;
            assert_eq!(taken(value, bit, true), Some(set));
            assert_eq!(taken(value, bit, false), Some(!set));
        }
    }
    assert_eq!(taken(0, 64, false), None);
}

#[test]
fn cfg_records_both_edges_and_preserves_nzcv() {
    let words = [
        0x6b01001fu32,
        encode(63, true, 0, 3),
        0x52800000,
        0xd65f03c0,
        0x52800020,
        0xd65f03c0,
    ];
    let mut seg = Segment::new(
        "text",
        0x1000,
        0,
        words.iter().flat_map(|w| w.to_le_bytes()).collect(),
    );
    let proc = Procedure::analyze_arm64(
        "test",
        0x1000,
        16,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    assert!(!proc.is_incomplete());
    assert_eq!(proc.basic_blocks[0].successors, vec![0x1010, 0x1008]);
    let text = proc.render_four_modes(&seg);
    assert!(text.contains("((x0 >> 63) & 1) != 0"));
    assert!(text.contains("loc_1010:"));
    assert!(!text.contains("unknown_nzcv_after_call"));
    let flags = ghidra_hopper_rs::arm64_flags::arithmetic(1, 2, 64, true)
        .unwrap()
        .1;
    for bits in 0..16 {
        let incoming = ghidra_hopper_rs::arm64_flags::Nzcv::from_bits(bits);
        let mut current = incoming;
        for op in decode_at(0x1000, encode(63, false, 0, 1)).pcode {
            match op {
                PcodeOp::TestBitBranch { bit, nonzero, .. } => {
                    assert_eq!(taken(1 << 63, bit, nonzero), Some(false))
                }
                PcodeOp::ClobberNzcv => current = flags,
                _ => panic!("bit branch must not change NZCV"),
            }
        }
        assert_eq!(current, incoming);
    }
}
