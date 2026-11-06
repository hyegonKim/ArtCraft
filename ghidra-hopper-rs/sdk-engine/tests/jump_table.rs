use ghidra_hopper_rs::{
    cfg_tags::Procedure,
    ghidra_port::{GhidraProgram, GhidraRefType, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{PcodeOp, SwitchHintTable},
    segment::{ByteType, Section, Segment},
};
use std::collections::{BTreeMap, BTreeSet};

fn put(seg: &mut Segment, pc: u64, word: u32) {
    let offset = (pc - seg.start_address) as usize;
    seg.data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
fn fixture() -> Segment {
    let mut seg = Segment::new("__TEXT", 0x1000, 0, vec![0; 0x208]);
    seg.permissions = Some(5);
    seg.sections = vec![
        Section {
            name: "__text".into(),
            start_address: 0x1000,
            length: 0x100,
            flags: 0x80000400,
        },
        Section {
            name: "__const".into(),
            start_address: 0x1200,
            length: 8,
            flags: 0,
        },
    ];
    for (pc, word) in [
        (0x1000, 0x9000001a), // adrp x26, 0x1000
        (0x1004, 0x9108035a), // add x26, x26, #0x200
        (0x1008, 0x2a0003e8), // mov w8, w0 (upper bits become zero)
        (0x100c, 0x71000d1f), // cmp w8, #3
        (0x1010, 0x54000208), // b.hi 0x1050
        (0x1014, 0x52800033), // mov w19, #1
        (0x1018, 0x10000089), // adr x9, 0x1028
        (0x101c, 0x78687b4a), // ldrh w10, [x26, x8, lsl #1]
        (0x1020, 0x8b0a0929), // add x9, x9, x10, lsl #2
        (0x1024, 0xd61f0120), // br x9
        (0x1028, 0x52800140),
        (0x102c, 0xd65f03c0),
        (0x1030, 0x52800160),
        (0x1034, 0xd65f03c0),
        (0x1038, 0x52800180),
        (0x103c, 0xd65f03c0),
        (0x1050, 0x52800c60),
        (0x1054, 0xd65f03c0),
    ] {
        put(&mut seg, pc, word);
    }
    for (n, half) in [0u16, 2, 0, 4].into_iter().enumerate() {
        seg.data[0x200 + n * 2..0x202 + n * 2].copy_from_slice(&half.to_le_bytes());
    }
    seg
}
fn analyze(seg: &mut Segment, budget: usize) -> Procedure {
    Procedure::analyze_arm64(
        "table",
        0x1000,
        budget,
        seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}
fn branch(p: &Procedure) -> &PcodeOp {
    &p.basic_blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| i.address == 0x1024)
        .unwrap()
        .pcode[0]
}
fn unresolved(seg: &mut Segment) {
    let p = analyze(seg, 128);
    assert!(
        matches!(branch(&p), PcodeOp::BranchInd { switch_targets, .. } if switch_targets.is_empty())
    );
    assert!(p.is_incomplete());
}

#[test]
fn reconstructs_index_mapping_edges_and_pseudocode_with_duplicates() {
    let mut seg = fixture();
    let p = analyze(&mut seg, 128);
    assert!(
        matches!(branch(&p), PcodeOp::TableBranch { index, table_address: 0x1200, offset_base: 0x1028, targets, .. } if index.offset == 8 && index.size == 4 && targets == &[0x1028, 0x1030, 0x1028, 0x1038])
    );
    let dispatch = p
        .basic_blocks
        .iter()
        .find(|b| b.instructions.last().unwrap().address == 0x1024)
        .unwrap();
    assert_eq!(dispatch.successors, vec![0x1028, 0x1030, 0x1038]);
    assert_eq!(
        seg.refs_from[&0x1024],
        BTreeSet::from([0x1028, 0x1030, 0x1038])
    );
    assert!(!p.is_incomplete());
    let view = p.render_four_modes(&seg);
    assert!(view.contains("switch (w8)"));
    assert!(view.contains("case 0: goto loc_1028;") && view.contains("case 2: goto loc_1028;"));
    assert!(!view.contains("goto *x9;"));
}

#[test]
fn recovered_cases_share_budget_and_missing_case_blocks_stay_incomplete() {
    let mut seg = fixture();
    let p = analyze(&mut seg, 13);
    assert_eq!(
        p.basic_blocks
            .iter()
            .map(|b| b.instructions.len())
            .sum::<usize>(),
        13
    );
    assert!(matches!(branch(&p), PcodeOp::TableBranch { .. }));
    assert!(p.is_incomplete());
    let p = analyze(&mut fixture(), 12);
    assert!(matches!(branch(&p), PcodeOp::BranchInd { .. }));
    assert!(p.is_incomplete());
}

#[test]
fn rejects_missing_bounds_wrong_condition_and_index_or_dispatch_clobbers() {
    for (pc, word) in [
        (0x1010, 0xd503201f), // missing guard
        (0x1010, 0x54000209), // b.ls selects the unbounded side
        (0x1014, 0x52800028), // mov w8, #1 overwrites the guarded index
        (0x1008, 0xaa0003e8), // mov x8, x0: CMP W8 does not bound upper bits
        (0x1020, 0x8b0a0d29), // scale is 8, not 4
        (0x101c, 0x78686b4a), // unscaled index aliases table entries
    ] {
        let mut seg = fixture();
        put(&mut seg, pc, word);
        unresolved(&mut seg);
    }
    let mut seg = fixture();
    // CBZ W0 -> ADR bypasses CMP/B.HI; a linear text scan would miss this edge.
    put(&mut seg, 0x1008, 0x34000080);
    unresolved(&mut seg);
}

#[test]
fn rejects_mutable_unknown_truncated_and_noncode_table_evidence() {
    for permissions in [None, Some(0), Some(1), Some(3), Some(7)] {
        let mut seg = fixture();
        seg.permissions = permissions;
        unresolved(&mut seg);
    }
    for half in [0x7fu16, 0xffff, 0xff] {
        let mut seg = fixture();
        seg.data[0x200..0x202].copy_from_slice(&half.to_le_bytes());
        unresolved(&mut seg);
    }
    let mut seg = fixture();
    seg.data.pop();
    unresolved(&mut seg);
    let mut seg = fixture();
    seg.sections[1].length = 6;
    unresolved(&mut seg);
    let mut seg = fixture();
    seg.sections[0].flags = 0;
    unresolved(&mut seg);
    let mut seg = fixture();
    seg.sections[1].flags = 1;
    unresolved(&mut seg);
    let mut seg = fixture();
    put(&mut seg, 0x100c, 0x713ffd1f);
    unresolved(&mut seg); // oversized bound
}

#[test]
fn late_backedge_that_clobbers_base_revokes_table_edges_and_code_states() {
    let mut seg = fixture();
    put(&mut seg, 0x1028, 0xd280001a); // newly found case overwrites x26
    put(&mut seg, 0x102c, 0x17fffff8); // b 0x100c
    unresolved(&mut seg);
    assert!(!seg.refs_from.contains_key(&0x1024));
    assert_eq!(seg.get_type_at_address(0x1028), ByteType::Undefined);
    let mut seg = fixture();
    put(&mut seg, 0x1028, 0x17fffffc); // newly found b 0x1018 bypasses range guard
    unresolved(&mut seg);
}

#[test]
fn benign_loop_and_unsupported_cases_preserve_their_distinct_status() {
    let mut seg = fixture();
    put(&mut seg, 0x1028, 0x2a0003e8); // w8 := w0
    put(&mut seg, 0x102c, 0x17fffff8); // b 0x100c
    let p = analyze(&mut seg, 128);
    assert!(matches!(branch(&p), PcodeOp::TableBranch { .. }));
    assert!(!p.is_incomplete());
    let mut seg = fixture();
    put(&mut seg, 0x1030, 0xffffffff);
    let p = analyze(&mut seg, 128);
    assert!(matches!(branch(&p), PcodeOp::TableBranch { .. }));
    assert!(p.is_incomplete());
    assert!(p.render_four_modes(&seg).contains("inst.arm64"));
}

#[test]
fn calls_preserve_callee_saved_base_but_revoke_caller_saved_base() {
    let mut seg = fixture();
    put(&mut seg, 0x1028, 0x94000032); // BL (callee body not followed)
    put(&mut seg, 0x102c, 0x2a0003e8); // reestablish zero-extended index after call
    put(&mut seg, 0x1030, 0x17fffff7); // b 0x100c
    let p = analyze(&mut seg, 128);
    assert!(matches!(branch(&p), PcodeOp::TableBranch { .. }));
    assert!(!p.is_incomplete());
    put(&mut seg, 0x1000, 0x9000000b); // same table through caller-saved x11
    put(&mut seg, 0x1004, 0x9108016b);
    put(&mut seg, 0x101c, 0x7868796a);
    unresolved(&mut seg);
}

#[test]
fn uxtw_index_uses_only_the_guarded_low_bits() {
    let mut seg = fixture();
    put(&mut seg, 0x1008, 0xaa0003e8); // x8 := x0, upper bits unknown
    put(&mut seg, 0x101c, 0x78685b4a); // [x26, w8, uxtw #1]
    let p = analyze(&mut seg, 128);
    assert!(matches!(branch(&p), PcodeOp::TableBranch { .. }));
    assert!(!p.is_incomplete());
}

#[test]
fn publishes_analysis_references_and_respects_explicit_overrides() {
    let mut program = GhidraProgram::new("table", fixture());
    let p = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut program,
        "table",
        0x1000,
        128,
        &[],
    );
    assert!(!p.is_incomplete());
    assert_eq!(
        program
            .references
            .iter()
            .filter(|r| r.from_addr == 0x1024 && r.ref_type == GhidraRefType::ComputedJump)
            .count(),
        3
    );
    assert!(program.references.iter().any(|r| r.from_addr == 0x1024
        && r.to_addr == 0x1200
        && r.ref_type == GhidraRefType::Data));
    let mut seg = fixture();
    let hints = SwitchHintTable {
        hints: BTreeMap::from([(0x1024, vec![0x1050])]),
    };
    let p = Procedure::analyze_arm64("override", 0x1000, 128, &mut seg, &BTreeMap::new(), &hints);
    assert!(
        matches!(branch(&p), PcodeOp::BranchInd { switch_targets, .. } if switch_targets == &[0x1050])
    );
    assert_eq!(seg.refs_from[&0x1024], BTreeSet::from([0x1050]));
}
