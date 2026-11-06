use ghidra_hopper_rs::{
    cfg_tags::Procedure,
    ghidra_port::{GhidraProgram, GhidraRefType, HopperHybridObjcAnalyzerRs},
    pcode_sleigh::{decode_and_lift_arm64, decompile_instructions_to_pseudocode, SwitchHintTable},
    segment::Segment,
};
use std::{collections::BTreeMap, process::Command};

#[test]
fn unsupported_instruction_is_visible_and_stops_cfg() {
    let mut seg = Segment::new(
        "__TEXT",
        0x1000,
        0,
        [0x9ac30842u32, 0xd65f03c0]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect(),
    );
    let proc = Procedure::analyze_arm64(
        "f",
        0x1000,
        8,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    let view = proc.render_four_modes(&seg);
    assert!(view.contains("INCOMPLETE"));
    assert!(!view.contains("return x0;"));
    assert!(proc.basic_blocks[0].successors.is_empty());
}

#[test]
fn unknown_receiver_never_creates_a_fake_external_target() {
    let mut prog = GhidraProgram::new(
        "test",
        Segment::new("__TEXT", 0x1000, 0, 0x94000004u32.to_le_bytes().to_vec()),
    );
    prog.objc_stub_selectors.insert(0x1010, "do:with:".into());
    let mut analyzer = HopperHybridObjcAnalyzerRs::new();
    analyzer.analyze_function_zero_ipc(&mut prog, "f", 0x1000, 1, &[]);
    assert!(prog.external_methods.is_empty());
    assert_eq!(analyzer.discovered_messages[0].receiver, "x0");
    assert_eq!(analyzer.discovered_messages[0].parameters, vec!["x2", "x3"]);
    assert!(prog
        .references
        .iter()
        .any(|r| r.to_addr == 0x1010 && r.ref_type == GhidraRefType::UnconditionalCall));
}

#[test]
fn cli_invalid_arguments_do_not_fall_back_to_demo_or_default_addresses() {
    for args in [
        vec!["not-a-mode"],
        vec!["decompile"],
        vec!["decompile", "/missing", "f", "nothex"],
        vec!["demo", "/missing"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ghidra-hopper-rs"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("bootstrapEngine"));
    }
}

#[test]
fn flags_setting_add_is_not_lifted_as_plain_add() {
    let seg = Segment::new("__TEXT", 0x1000, 0, vec![0; 16]);
    let insn = decode_and_lift_arm64(
        0x1000,
        0xb1000400,
        &seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    assert_eq!(insn.mnemonic, "adds");
    assert!(matches!(
        insn.pcode[0],
        ghidra_hopper_rs::pcode_sleigh::PcodeOp::FlagArithmetic { .. }
    ));
}

#[test]
fn unknown_instruction_cannot_preserve_a_stale_string_argument() {
    let mut data = vec![0; 128];
    data[64..68].copy_from_slice(b"old\0");
    let seg = Segment::new("__TEXT", 0x1000, 0, data);
    let words = [0x90000002, 0x91010042, 0x9ac30842, 0x94000004];
    let stubs = BTreeMap::from([(0x101c, "go:".into())]);
    let insns: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            decode_and_lift_arm64(
                0x1000 + i as u64 * 4,
                w,
                &seg,
                &stubs,
                &SwitchHintTable::default(),
            )
        })
        .collect();
    let text = decompile_instructions_to_pseudocode("f", 0x1000, &insns, &seg);
    assert!(text.contains("INCOMPLETE"));
    assert!(!text.contains("[x0 go:\"old\"]"));
}

#[test]
fn multiple_objc_arguments_and_call_clobbers_are_preserved() {
    let mut data = vec![0; 68];
    data[64..68].copy_from_slice(b"old\0");
    let seg = Segment::new("__TEXT", 0x1000, 0, data);
    let words = [0x90000002, 0x91010042, 0x94000004, 0x94000004];
    let stubs = BTreeMap::from([(0x1018, "first:second:".into()), (0x101c, "later:".into())]);
    let insns: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, &word)| {
            decode_and_lift_arm64(
                0x1000 + i as u64 * 4,
                word,
                &seg,
                &stubs,
                &SwitchHintTable::default(),
            )
        })
        .collect();
    let text = decompile_instructions_to_pseudocode("f", 0x1000, &insns, &seg);
    assert!(text.contains("[x0 first:\"old\" second:x3]"));
    assert!(text.contains("[x0 later:x2]"));
    assert!(!text.contains("@\"old\""));
}

#[test]
fn nzcv_conditions_do_not_turn_into_invented_registers() {
    let mut seg = Segment::new("__TEXT", 0x1000, 0, 0x54000040u32.to_le_bytes().to_vec());
    let proc = Procedure::analyze_arm64(
        "f",
        0x1000,
        1,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    let text = proc.render_four_modes(&seg);
    assert!(text.contains("INCOMPLETE"));
    assert!(!text.contains("if (x100)"));
}
