use ghidra_hopper_rs::{
    arm64_address::evaluate,
    cfg_tags::Procedure,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
fn word(immediate: u32, rd: u32, page: bool) -> u32 {
    0x10000000 | ((page as u32) << 31) | ((immediate & 3) << 29) | ((immediate >> 2) << 5) | rd
}
fn decode(pc: u64, w: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        pc,
        w,
        &Segment::new("text", pc, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}

#[test]
fn hopper_adr_computes_address_not_memory_or_control_flow() {
    let insn = decode(0x100003258, 0x10000089);
    assert_eq!(insn.mnemonic, "adr");
    assert_eq!(insn.branch_target, None);
    assert!(
        !insn.is_call && !insn.is_ret && !insn.is_conditional_jump && !insn.is_unconditional_jump
    );
    assert_eq!(
        insn.pcode,
        vec![PcodeOp::Copy {
            dst: ghidra_hopper_rs::pcode_sleigh::Varnode::reg(9, 8),
            src: ghidra_hopper_rs::pcode_sleigh::Varnode::imm(0x100003268, 8)
        }]
    );
}

#[test]
fn every_signed_21_bit_displacement_matches_widened_addition() {
    for immediate in 0..1 << 21 {
        let signed = if immediate < 1 << 20 {
            immediate as i128
        } else {
            immediate as i128 - (1 << 21)
        };
        for page in [false, true] {
            let pc = 0x100001ffc;
            let base = if page { pc & !0xfff } else { pc };
            let delta = if page { signed * 4096 } else { signed };
            assert_eq!(
                evaluate(pc, immediate, page),
                Some((base as i128 + delta) as u64)
            );
        }
    }
    assert_eq!(evaluate(0, 1 << 21, false), None);
}

#[test]
fn immediate_fields_registers_pc_wrap_and_page_granularity_are_correct() {
    for immediate in [
        0, 1, 2, 3, 4, 0xfffff, 0x100000, 0x1ffffc, 0x1ffffd, 0x1ffffe, 0x1fffff,
    ] {
        for rd in 0..32 {
            for pc in [0, 0xffc, 0x100000004, u64::MAX - 3] {
                for page in [false, true] {
                    let insn = decode(pc, word(immediate, rd, page));
                    match insn.pcode[0] {
                        PcodeOp::Copy { dst, src } => {
                            assert_eq!(src.space, SpaceKind::Constant);
                            assert_eq!(src.offset, evaluate(pc, immediate, page).unwrap());
                            assert_eq!(dst.size, 8);
                            assert_eq!(
                                dst.space,
                                if rd == 31 {
                                    SpaceKind::Unique
                                } else {
                                    SpaceKind::Register
                                }
                            );
                            if rd != 31 {
                                assert_eq!(dst.offset, rd as u64);
                            }
                        }
                        _ => panic!("address formation must lower to Copy"),
                    }
                }
            }
        }
    }
    assert_eq!(evaluate(u64::MAX, 1, false), Some(0));
    assert_eq!(evaluate(0, 0x1fffff, false), Some(u64::MAX));
    assert_eq!(evaluate(0xffc, 1, false), Some(0xffd));
    assert_eq!(evaluate(0xffc, 1, true), Some(0x1000));
}

#[test]
fn adr_continues_to_next_instruction_and_preserves_flags() {
    let words = [0x6b01001fu32, 0x10000049, 0xd65f03c0];
    let mut seg = Segment::new(
        "text",
        0x1000,
        0,
        words.iter().flat_map(|w| w.to_le_bytes()).collect(),
    );
    let proc = Procedure::analyze_arm64(
        "test",
        0x1000,
        8,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    assert!(!proc.is_incomplete());
    assert_eq!(proc.basic_blocks[0].instructions.len(), 3);
    let text = proc.render_four_modes(&seg);
    assert!(text.contains("x9 = 0x100c;"));
    assert!(!text.contains("unknown_nzcv_after_call"));
    assert_eq!(decode(0x1000, 0x10000049).pcode.len(), 1);
    assert_eq!(decode(0x1000, 0x1000005f).formatted_args[0], "xzr");
    assert_eq!(decode(0x1000, 0x9000001f).formatted_args[0], "xzr");
}
