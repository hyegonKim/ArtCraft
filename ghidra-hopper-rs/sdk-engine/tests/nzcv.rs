use ghidra_hopper_rs::{
    arm64_flags::{arithmetic, shifted, Condition, Nzcv, Shift},
    cfg_tags::Procedure,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;
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
fn arithmetic_flags_match_widened_integer_oracle() {
    for width in [32u8, 64] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            u32::MAX as u64
        };
        let sign = 1u64 << (width - 1);
        let values = [0, 1, 2, sign - 1, sign, sign + 1, mask - 1, mask];
        for a in values {
            for b in values {
                for subtract in [false, true] {
                    let (result, f) = arithmetic(a, b, width, subtract).unwrap();
                    let signed = |x: u64| {
                        if x & sign != 0 {
                            x as i128 - (1i128 << width)
                        } else {
                            x as i128
                        }
                    };
                    let exact = if subtract {
                        signed(a) - signed(b)
                    } else {
                        signed(a) + signed(b)
                    };
                    let unsigned = if subtract {
                        a as i128 - b as i128
                    } else {
                        a as i128 + b as i128
                    };
                    assert_eq!(result, (unsigned & mask as i128) as u64);
                    assert_eq!(f.n, result & sign != 0);
                    assert_eq!(f.z, result == 0);
                    assert_eq!(
                        f.c,
                        if subtract {
                            unsigned >= 0
                        } else {
                            unsigned > mask as i128
                        }
                    );
                    assert_eq!(
                        f.v,
                        exact < -(1i128 << (width - 1)) || exact > (1i128 << (width - 1)) - 1
                    );
                    for (condition, expected) in [
                        (0, a == b),
                        (1, a != b),
                        (2, a >= b),
                        (3, a < b),
                        (8, a > b),
                        (9, a <= b),
                        (10, signed(a) >= signed(b)),
                        (11, signed(a) < signed(b)),
                        (12, signed(a) > signed(b)),
                        (13, signed(a) <= signed(b)),
                    ] {
                        if subtract {
                            assert_eq!(Condition::from_code(condition).unwrap().holds(f), expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn all_conditions_and_shifts_cover_boundary_cases() {
    for bits in 0..16 {
        let f = Nzcv::from_bits(bits);
        assert_eq!(f.bits(), bits);
        for code in (0..14).step_by(2) {
            assert_ne!(
                Condition::from_code(code).unwrap().holds(f),
                Condition::from_code(code + 1).unwrap().holds(f)
            );
        }
        assert!(Condition::from_code(14).unwrap().holds(f));
        assert!(Condition::from_code(15).unwrap().holds(f));
    }
    assert_eq!(shifted(0x80000000, 32, Shift::Asr, 31), Some(0xffffffff));
    assert_eq!(shifted(0x80000000, 32, Shift::Lsr, 31), Some(1));
    assert_eq!(shifted(u64::MAX, 32, Shift::Lsl, 31), Some(0x80000000));
    assert_eq!(shifted(1, 64, Shift::Lsl, 63), Some(1 << 63));
    assert_eq!(shifted(1, 32, Shift::Lsl, 32), None);
}

#[test]
fn cmp_register_zero_and_immediate_sp_are_distinct() {
    assert_eq!(decode(0x6b14011f).mnemonic, "cmp");
    assert!(
        matches!(decode(0x6b14011f).pcode[0],PcodeOp::FlagArithmetic{dst:None,lhs,rhs,subtract:true,..} if lhs.offset==8 && rhs.offset==20 && lhs.size==4)
    );
    assert!(
        matches!(decode(0xf10003ff).pcode[0],PcodeOp::FlagArithmetic{lhs,..} if lhs.space==SpaceKind::Register && lhs.offset==31)
    );
    assert!(
        matches!(decode(0xeb0003ff).pcode[0],PcodeOp::FlagArithmetic{lhs,..} if lhs.space==SpaceKind::Constant && lhs.offset==0)
    );
    for word in [0x6bc0001f, 0x6b00801f, 0x54000010] {
        assert!(
            matches!(decode(word).pcode[0], PcodeOp::Unsupported { .. }),
            "{word:x}"
        );
    }
}

#[test]
fn cmp_and_conditional_branch_have_both_edges_and_explicit_state() {
    let mut seg = Segment::new(
        "text",
        0x1000,
        0,
        [
            0x6b01001fu32,
            0x54000063,
            0x528000e0,
            0xd65f03c0,
            0x52800060,
            0xd65f03c0,
        ]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect(),
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
    assert!(text.contains("arm64_subs_u32"));
    assert!(text.contains("if (!nzcv.C)"));
    assert!(!text.contains("x103"));
}

#[test]
fn calls_invalidate_nzcv_but_non_flag_instructions_preserve_it() {
    for word in [0x94000004, 0xd63f0100] {
        assert!(matches!(
            decode(word).pcode.last(),
            Some(PcodeOp::ClobberNzcv)
        ));
    }
    for word in [0x91000400, 0xaa0103e0, 0xd503201f, 0xb4000020] {
        assert!(!decode(word)
            .pcode
            .iter()
            .any(|op| matches!(op, PcodeOp::ClobberNzcv | PcodeOp::FlagArithmetic { .. })));
    }
}

#[test]
fn al_and_nv_are_unconditional_and_have_no_false_fallthrough() {
    for word in [0x5400004e, 0x5400004f] {
        let d = decode(word);
        assert!(d.is_unconditional_jump);
        assert!(!d.is_conditional_jump);
        assert_eq!(d.branch_target, Some(0x1008));
    }
    assert_eq!(decode(0x54ffffe0).branch_target, Some(0xffc));
}

#[test]
fn flags_are_latched_before_register_writes_and_unknown_after_calls() {
    let mut regs = [0u64; 32];
    regs[0] = 5;
    regs[1] = 5;
    let mut flags = None;
    let mut execute = |word| {
        for op in decode(word).pcode {
            let read = |v: ghidra_hopper_rs::pcode_sleigh::Varnode| {
                if v.space == SpaceKind::Constant {
                    v.offset
                } else {
                    regs[v.offset as usize]
                }
            };
            match op {
                PcodeOp::FlagArithmetic {
                    dst,
                    lhs,
                    rhs,
                    shift,
                    amount,
                    subtract,
                } => {
                    let shifted = shifted(read(rhs), rhs.size * 8, shift, amount).unwrap();
                    let (result, nzcv) =
                        arithmetic(read(lhs), shifted, lhs.size * 8, subtract).unwrap();
                    if let Some(dst) = dst {
                        regs[dst.offset as usize] = result;
                    }
                    flags = Some(nzcv);
                }
                PcodeOp::Copy { dst, src } => regs[dst.offset as usize] = read(src),
                PcodeOp::ClobberNzcv => flags = None,
                PcodeOp::FlagBranch { condition, .. } => return flags.map(|f| condition.holds(f)),
                _ => {}
            }
        }
        None
    };
    execute(0x6b01001f); // cmp w0,w1: Z=1
    execute(0x528000e1); // mov w1,#7 must not change latched flags
    assert_eq!(execute(0x54000040), Some(true));
    execute(0x31000400); // adds w0,w0,#1 overwrites flags, result=6
    assert_eq!(execute(0x54000040), Some(false));
    execute(0x94000004); // unknown callee can clobber NZCV
    assert_eq!(execute(0x54000040), None);
}
