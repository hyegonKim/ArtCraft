use ghidra_hopper_rs::{
    arm64_bitfield::evaluate,
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    macho_objc::MachOImage,
    pcode_sleigh::{decode_and_lift_arm64, BitfieldKind, PcodeOp, SwitchHintTable},
    segment::Segment,
};
use std::collections::BTreeMap;

#[test]
fn bitfields_match_extract_insert_and_sign_extension_oracle() {
    // An independent piecewise oracle uses extraction/insertion, without rotated masks.
    for width in [32u8, 64] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            u32::MAX as u64
        };
        for r in 0..width {
            for s in 0..width {
                for source in [0u64, u64::MAX, 0x8123456789abcdef] {
                    let old = 0xdeadbeef01234567 & mask;
                    let count = if s >= r { s - r + 1 } else { s + 1 };
                    let lowmask = if count == 64 {
                        u64::MAX
                    } else {
                        (1u64 << count) - 1
                    };
                    let shift = if s >= r { 0 } else { width - r };
                    let field = if s >= r { (source & mask) >> r } else { source };
                    let field = (field & lowmask) << shift;
                    let unsigned = field & mask;
                    let signpos = shift + count - 1;
                    let signed = if field & (1u64 << signpos) != 0 {
                        field | mask.checked_shl((signpos + 1) as u32).unwrap_or(0)
                    } else {
                        field
                    };
                    let insert = (old & !(lowmask << shift)) | field;
                    assert_eq!(
                        evaluate(BitfieldKind::Unsigned, width, r, s, source, old),
                        Some(unsigned)
                    );
                    assert_eq!(
                        evaluate(BitfieldKind::Signed, width, r, s, source, old),
                        Some(signed & mask)
                    );
                    assert_eq!(
                        evaluate(BitfieldKind::Insert, width, r, s, source, old),
                        Some(insert & mask)
                    );
                }
            }
        }
    }
}

#[test]
fn hopper_lsl_alias_and_reserved_encodings() {
    let seg = Segment::new("text", 0x1000, 0, vec![0; 16]);
    let d = |w| {
        decode_and_lift_arm64(
            0x1000,
            w,
            &seg,
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        )
    };
    assert!(matches!(
        d(0x53081c08).pcode[0],
        PcodeOp::Bitfield {
            kind: BitfieldKind::Unsigned,
            immr: 8,
            imms: 7,
            ..
        }
    ));
    assert_eq!(
        evaluate(BitfieldKind::Unsigned, 32, 8, 7, 0x12345678, 0),
        Some(0x78000000)
    );
    for word in [0x53400000, 0x53200000, 0x93000000, 0x73000000] {
        assert!(
            matches!(d(word).pcode[0], PcodeOp::Unsupported { .. }),
            "{word:x}"
        );
    }
    assert_eq!(
        evaluate(BitfieldKind::Unsigned, 64, 0, 63, u64::MAX, 0),
        Some(u64::MAX)
    );
    assert_eq!(evaluate(BitfieldKind::Unsigned, 32, 32, 0, 1, 0), None);
}

fn p32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn p64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
fn elf() -> Vec<u8> {
    let mut b = vec![0; 0x800];
    b[..6].copy_from_slice(b"\x7fELF\x02\x01");
    b[18..20].copy_from_slice(&183u16.to_le_bytes());
    p64(&mut b, 32, 64);
    p64(&mut b, 40, 128);
    for (off, value) in [(54, 56u16), (56, 1), (58, 64), (60, 6), (62, 1)] {
        b[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }
    p32(&mut b, 64, 1);
    p64(&mut b, 80, 0x1000);
    p64(&mut b, 96, 0x800);
    let names = b"\0.shstrtab\0.dynstr\0.dynsym\0.rela.plt\0.plt\0";
    b[0x220..0x220 + names.len()].copy_from_slice(names);
    for (idx, name, typ, addr, off, size, link, ent) in [
        (1, ".shstrtab", 3, 0, 0x220, names.len(), 0, 0),
        (2, ".dynstr", 3, 0, 0x280, 9, 0, 0),
        (3, ".dynsym", 11, 0, 0x300, 72, 2, 24),
        (4, ".rela.plt", 4, 0, 0x380, 48, 3, 24),
        (5, ".plt", 1, 0x1400, 0x400, 32, 0, 0),
    ] {
        let sh = 128 + idx * 64;
        let n = names
            .windows(name.len())
            .position(|w| w == name.as_bytes())
            .unwrap();
        p32(&mut b, sh, n as u32);
        p32(&mut b, sh + 4, typ);
        p64(&mut b, sh + 16, addr);
        p64(&mut b, sh + 24, off);
        p64(&mut b, sh + 32, size as u64);
        p32(&mut b, sh + 40, link);
        p64(&mut b, sh + 56, ent);
    }
    b[0x280..0x289].copy_from_slice(b"\0foo\0bar\0");
    p32(&mut b, 0x318, 1);
    p32(&mut b, 0x330, 5);
    // Deliberately reverse relocation order relative to the actual stub order.
    p64(&mut b, 0x380, 0x1608);
    p64(&mut b, 0x388, (2u64 << 32) | 1026);
    p64(&mut b, 0x398, 0x1600);
    p64(&mut b, 0x3a0, (1u64 << 32) | 1026);
    for (off, disp) in [(0x400, 0x600u32), (0x410, 0x608)] {
        p32(&mut b, off, 0x90000010);
        p32(&mut b, off + 4, 0xf9400211 | ((disp / 8) << 10));
        p32(&mut b, off + 8, 0x91000210 | (disp << 10));
        p32(&mut b, off + 12, 0xd61f0220);
    }
    p64(&mut b, 0x600, 0x1400); // lazy GOT value is deliberately not a final function address
    b
}

#[test]
fn plt_matches_got_relocations_not_ordinal_or_lazy_pointer() {
    let image = MachOImage::parse(&elf()).unwrap();
    assert_eq!(image.objc.plt_bindings[&0x140c].symbol, "foo");
    assert_eq!(image.objc.plt_bindings[&0x141c].symbol, "bar");
    let mut prog = GhidraProgram::from_macho_image("test", &image);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut prog,
        "plt",
        0x1400,
        16,
        &[],
    );
    assert_eq!(proc.basic_blocks[0].instructions.len(), 4);
    assert!(!proc.is_incomplete());
    assert!(
        matches!(proc.basic_blocks[0].instructions[3].pcode[0],PcodeOp::ExternalTailCall{ref symbol,got_address:0x1600} if symbol=="foo")
    );
    assert!(prog.external_methods.is_empty());
    assert!(proc
        .render_four_modes(prog.segment_at(0x1400).unwrap())
        .contains("runtime address and signature unknown"));
}

#[test]
fn corrupt_relocations_or_stub_code_remain_unresolved() {
    for mutation in 0..6 {
        let mut b = elf();
        match mutation {
            0 => p64(&mut b, 0x3a0, (1u64 << 32) | 1025), // GLOB_DAT is not JUMP_SLOT
            1 => p64(&mut b, 0x3a8, 1),                   // unsupported addend
            2 => p32(&mut b, 0x408, 0x91000610),          // ADD does not match LDR's GOT slot
            3 => p32(&mut b, 128 + 4 * 64 + 40, 2),       // relocation linked to wrong table
            4 => p64(&mut b, 0x398, 0x1700),              // no relocation for the actual GOT slot
            _ => p64(&mut b, 128 + 5 * 64 + 16, 0x1500),  // section VA does not match mapped code
        }
        let image = MachOImage::parse(&b).unwrap();
        assert!(
            !image.objc.plt_bindings.contains_key(&0x140c),
            "mutation {mutation}"
        );
        let mut prog = GhidraProgram::from_macho_image("test", &image);
        let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
            &mut prog,
            "plt",
            0x1400,
            16,
            &[],
        );
        assert!(proc.is_incomplete());
    }
}

#[test]
fn jumping_into_middle_of_plt_does_not_prove_got_load() {
    let image = MachOImage::parse(&elf()).unwrap();
    let mut prog = GhidraProgram::from_macho_image("test", &image);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut prog,
        "middle",
        0x140c,
        8,
        &[],
    );
    assert!(proc.is_incomplete());
}

#[test]
fn changed_plt_code_cannot_reuse_loaded_binding_metadata() {
    let image = MachOImage::parse(&elf()).unwrap();
    let mut prog = GhidraProgram::from_macho_image("changed", &image);
    p32(&mut prog.segments[0].data, 0x404, 0xd503201f);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut prog,
        "changed",
        0x1400,
        16,
        &[],
    );
    assert!(proc.is_incomplete());
    assert!(matches!(
        proc.basic_blocks[0].instructions[3].pcode[0],
        PcodeOp::BranchInd { .. }
    ));
}

#[test]
fn stack_failure_call_stops_before_adjacent_function_only_with_live_binding() {
    let mut b = elf();
    let names = b"\0__stack_chk_fail\0bar\0";
    b[0x280..0x280 + names.len()].copy_from_slice(names);
    p64(&mut b, 128 + 2 * 64 + 32, names.len() as u64);
    p32(&mut b, 0x330, 18);
    for (at, w) in [
        (0x200, 0x94000080),
        (0x204, 0x52800020),
        (0x208, 0xd65f03c0),
    ] {
        p32(&mut b, at, w);
    }
    let image = MachOImage::parse(&b).unwrap();
    let mut prog = GhidraProgram::from_macho_image("fail", &image);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut prog,
        "fail",
        0x1200,
        16,
        &[],
    );
    assert_eq!(proc.basic_blocks[0].instructions.len(), 1);
    assert!(proc.basic_blocks[0].successors.is_empty());
    assert!(!proc.is_incomplete());
    assert!(matches!(
        proc.basic_blocks[0].instructions[0].pcode[0],
        PcodeOp::NoReturnCall { .. }
    ));
    let mut prog = GhidraProgram::from_macho_image("changed", &image);
    p32(&mut prog.segments[0].data, 0x404, 0xd503201f);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut prog,
        "changed",
        0x1200,
        16,
        &[],
    );
    assert_eq!(proc.basic_blocks[0].instructions.len(), 3);
    assert!(!matches!(
        proc.basic_blocks[0].instructions[0].pcode[0],
        PcodeOp::NoReturnCall { .. }
    ));
}

fn proof_program(callee: &[u32], multi_call: bool) -> GhidraProgram {
    let mut b = elf();
    let names = b"\0__stack_chk_fail\0bar\0";
    b[0x280..0x280 + names.len()].copy_from_slice(names);
    p64(&mut b, 128 + 2 * 64 + 32, names.len() as u64);
    p32(&mut b, 0x330, 18);
    p32(&mut b, 0x200, 0x94000014); // BL 0x1250
    if multi_call {
        p32(&mut b, 0x204, 0xd503201f);
        p32(&mut b, 0x208, 0x94000012);
    }
    for (n, w) in callee.iter().enumerate() {
        p32(&mut b, 0x250 + n * 4, *w);
    }
    let image = MachOImage::parse(&b).unwrap();
    let mut p = GhidraProgram::from_macho_image("proof", &image);
    p.segments[0].function_starts.extend([
        0x1200,
        if multi_call { 0x120c } else { 0x1204 },
        0x1250,
    ]);
    p
}
#[test]
fn callee_proof_is_complete_nonreturning_and_charged_to_one_budget() {
    for (budget, proven) in [(1, false), (2, true), (8, true)] {
        let mut p = proof_program(&[0x9400006c], false);
        let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
            &mut p,
            "proof",
            0x1200,
            budget,
            &[],
        );
        assert_eq!(!f.is_incomplete(), proven);
        assert!(f.analysis_work <= budget);
        if proven {
            assert_eq!(f.analysis_work, 2);
            assert_eq!(f.nonreturning_proofs[&0x1250], (1, true));
        }
    }
    for (budget, proven) in [(2, false), (3, true)] {
        let mut p = proof_program(&[0xd503201f, 0x9400006b], false);
        let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
            &mut p,
            "proof",
            0x1200,
            budget,
            &[],
        );
        assert_eq!(!f.is_incomplete(), proven);
        assert_eq!(f.analysis_work, budget);
    }
}
#[test]
fn normal_return_unknown_opcode_or_closed_loop_is_not_a_noreturn_proof() {
    for code in [
        vec![0xd65f03c0],
        vec![0xffffffff],
        vec![0x14000000],
        vec![0x34000080, 0x9400006b, 0xd503201f, 0xd503201f, 0xd65f03c0],
    ] {
        let mut p = proof_program(&code, false);
        let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
            &mut p,
            "proof",
            0x1200,
            16,
            &[],
        );
        assert!(f.is_incomplete());
        assert!(!f.nonreturning_proofs[&0x1250].1);
    }
}
#[test]
fn proven_calls_prune_cached_caller_without_hidden_redecode_or_stale_refs() {
    let mut p = proof_program(&[0x9400006c], true);
    let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut p,
        "proof",
        0x1200,
        8,
        &[],
    );
    assert!(!f.is_incomplete());
    assert_eq!(f.analysis_work, 4);
    assert_eq!(f.basic_blocks[0].instructions.len(), 1);
    let seg = &p.segments[0];
    assert_eq!(
        seg.get_type_at_address(0x1204),
        ghidra_hopper_rs::segment::ByteType::Undefined
    );
    assert!(!seg.refs_from.contains_key(&0x1208));
    assert!(seg.refs_from[&0x1200].contains(&0x1250));
}
