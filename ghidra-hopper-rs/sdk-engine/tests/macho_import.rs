use ghidra_hopper_rs::{
    ghidra_port::{GhidraJumpTable, GhidraProgram, GhidraRefType, HopperHybridObjcAnalyzerRs},
    macho_objc::MachOImage,
    pcode_sleigh::PcodeOp,
};

fn p32(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn p64(b: &mut [u8], at: usize, n: u64) {
    b[at..at + 8].copy_from_slice(&n.to_le_bytes());
}
fn segment(b: &mut [u8], at: usize, vm: u64, file: u64, size: u64, prot: u32) {
    p32(b, at, 0x19);
    p32(b, at + 4, 152);
    p64(b, at + 24, vm);
    p64(b, at + 32, 0x1000);
    p64(b, at + 40, file);
    p64(b, at + 48, size);
    p32(b, at + 56, prot);
    p32(b, at + 60, prot);
    p32(b, at + 64, 1);
}

fn fixture() -> Vec<u8> {
    let mut b = vec![0; 0x700];
    p32(&mut b, 0, 0xfeedfacf);
    p32(&mut b, 4, 0x100000c);
    p32(&mut b, 12, 2);
    p32(&mut b, 16, 5);
    p32(&mut b, 20, 464);
    p32(&mut b, 24, 0x84);
    segment(&mut b, 32, 0x1000, 0, 0x300, 5);
    b[40..46].copy_from_slice(b"__TEXT");
    let s = 104;
    b[s..s + 7].copy_from_slice(b"__stubs");
    p64(&mut b, s + 32, 0x1200);
    p64(&mut b, s + 40, 12);
    p32(&mut b, s + 48, 0x200);
    p32(&mut b, s + 64, 0x80000408);
    p32(&mut b, s + 72, 12);
    segment(&mut b, 184, 0x2000, 0x400, 0x100, 3);
    b[192..198].copy_from_slice(b"__DATA");
    let s = 256;
    b[s..s + 15].copy_from_slice(b"__nl_symbol_ptr");
    p64(&mut b, s + 32, 0x2000);
    p64(&mut b, s + 40, 8);
    p32(&mut b, s + 48, 0x400);
    p32(&mut b, s + 64, 6);
    p32(&mut b, s + 68, 1);
    p32(&mut b, 336, 2);
    p32(&mut b, 340, 24); // LC_SYMTAB
    p32(&mut b, 344, 0x500);
    p32(&mut b, 348, 2);
    p32(&mut b, 352, 0x540);
    p32(&mut b, 356, 32);
    p32(&mut b, 360, 11);
    p32(&mut b, 364, 80); // LC_DYSYMTAB
    p32(&mut b, 416, 0x580);
    p32(&mut b, 420, 2);
    p32(&mut b, 440, 0xc);
    p32(&mut b, 444, 56);
    p32(&mut b, 448, 24);
    let library = b"/usr/lib/libFixture.dylib\0";
    b[464..464 + library.len()].copy_from_slice(library);
    for (at, word) in [
        (0x200, 0xb0000010),
        (0x204, 0xf9400210),
        (0x208, 0xd61f0200),
        (0x210, 0xd65f03c0),
    ] {
        p32(&mut b, at, word);
    }
    p64(&mut b, 0x400, 0xdeadc0de00000000); // never interpreted as a code address
    p32(&mut b, 0x500, 1);
    b[0x504] = 1;
    b[0x507] = 1;
    p32(&mut b, 0x510, 6);
    b[0x514] = 1;
    b[0x517] = 1;
    b[0x540..0x54b].copy_from_slice(b"\0_foo\0_bar\0");
    b
}
fn analyze(
    b: &[u8],
    entry: u64,
    budget: usize,
) -> (GhidraProgram, ghidra_hopper_rs::cfg_tags::Procedure) {
    let image = MachOImage::parse(b).unwrap();
    let mut program = GhidraProgram::from_macho_image("import", &image);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut program,
        "import",
        entry,
        budget,
        &[],
    );
    (program, proc)
}
fn unresolved(b: &[u8]) {
    let image = MachOImage::parse(b).unwrap();
    assert!(image.objc.macho_imports.is_empty());
    let (_, proc) = analyze(b, 0x1200, 16);
    assert!(proc.is_incomplete());
    assert!(proc
        .basic_blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .all(|i| !matches!(i.pcode.first(), Some(PcodeOp::MachOImportTailCall { .. }))));
}

#[test]
fn links_stub_slot_symbol_and_dependency_without_inventing_runtime_address() {
    let b = fixture();
    let image = MachOImage::parse(&b).unwrap();
    let binding = &image.objc.macho_imports[&0x1208];
    assert_eq!(binding.stub_address, 0x1200);
    assert_eq!(binding.pointer_address, 0x2000);
    assert_eq!(binding.symbol, "_foo");
    assert_eq!(binding.library, "/usr/lib/libFixture.dylib");
    let (program, proc) = analyze(&b, 0x1200, 3);
    assert!(!proc.is_incomplete());
    assert!(
        matches!(&proc.basic_blocks[0].instructions[2].pcode[0], PcodeOp::MachOImportTailCall { symbol, library, pointer_address: 0x2000, .. } if symbol == "_foo" && library.ends_with("libFixture.dylib"))
    );
    assert!(proc.basic_blocks[0].successors.is_empty());
    assert!(program.references.iter().any(|r| r.from_addr == 0x1208
        && r.to_addr == 0x2000
        && r.ref_type == GhidraRefType::Data));
    assert!(program.external_methods.is_empty() && program.objc_stub_selectors.is_empty());
    let view = proc.render_four_modes(program.text_segment().unwrap());
    assert!(view.contains("tailcall_import(\"_foo\""));
    assert!(view.contains("runtime address and signature unknown"));
    assert!(!view.contains("JUMP_SLOT") && !view.contains("deadc0de"));
}

#[test]
fn pointer_initial_values_and_lazy_slot_type_do_not_change_symbol_identity() {
    for value in [0, 0x1200, u64::MAX, 0xdeadbeef00000000] {
        let mut b = fixture();
        p64(&mut b, 0x400, value);
        let (_, proc) = analyze(&b, 0x1200, 16);
        assert!(!proc.is_incomplete());
    }
    let mut b = fixture();
    p32(&mut b, 256 + 64, 7);
    let (_, proc) = analyze(&b, 0x1200, 16);
    assert!(!proc.is_incomplete());
}

#[test]
fn mid_stub_entries_budgets_and_changed_instructions_do_not_inherit_proof() {
    for (entry, budget) in [(0x1204, 16), (0x1208, 16), (0x1200, 1), (0x1200, 2)] {
        let (_, proc) = analyze(&fixture(), entry, budget);
        assert!(proc.is_incomplete());
    }
    let image = MachOImage::parse(&fixture()).unwrap();
    let mut program = GhidraProgram::from_macho_image("changed", &image);
    p32(&mut program.segments[0].data, 0x204, 0xd503201f);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut program,
        "changed",
        0x1200,
        16,
        &[],
    );
    assert!(proc.is_incomplete());
    assert!(matches!(
        proc.basic_blocks[0].instructions[2].pcode[0],
        PcodeOp::BranchInd { .. }
    ));
}

#[test]
fn requires_matching_indices_even_when_symbol_names_are_equal() {
    let mut b = fixture();
    p32(&mut b, 0x584, 1);
    unresolved(&b);
    p32(&mut b, 0x510, 1);
    unresolved(&b); // different nlist entries, same name
    for value in [0x80000000, 0x40000000, 0xc0000000, 2, u32::MAX] {
        let mut b = fixture();
        p32(&mut b, 0x580, value);
        p32(&mut b, 0x584, value);
        unresolved(&b);
    }
}

#[test]
fn refuses_unsupported_stubs_bad_mapping_permissions_and_reserved_indices() {
    for (at, value) in [
        (0x208, 0xd61f0220),
        (0x204, 0xb9400210),
        (0x200, 0xb0000011),
        (104 + 72, 16),
        (104 + 68, u32::MAX),
        (256 + 68, 100),
        (32 + 60, 7),
        (184 + 60, 0),
        (104 + 48, 0x220),
        (256 + 48, 0x408),
    ] {
        let mut b = fixture();
        p32(&mut b, at, value);
        unresolved(&b);
    }
    let mut b = fixture();
    p32(&mut b, 24, 4);
    unresolved(&b); // flat namespace
    let mut b = fixture();
    let code = b[0x200..0x20c].to_vec();
    b[0x220..0x22c].copy_from_slice(&code);
    p32(&mut b, 104 + 48, 0x220);
    unresolved(&b); // identical bytes do not establish the section/segment mapping
    let mut b = fixture();
    let pointer = b[0x400..0x408].to_vec();
    b[0x408..0x410].copy_from_slice(&pointer);
    p32(&mut b, 256 + 48, 0x408);
    unresolved(&b);
}

#[test]
fn refuses_weak_defined_anonymous_and_unscoped_imports() {
    for desc in [0u16, 0x200, 0xfe00, 0xff00, 0x140] {
        let mut b = fixture();
        b[0x506..0x508].copy_from_slice(&desc.to_le_bytes());
        unresolved(&b);
    }
    for kind in [0, 0xf, 0x11, 0xe1] {
        let mut b = fixture();
        b[0x504] = kind;
        unresolved(&b);
    }
    let mut b = fixture();
    b[0x505] = 1;
    unresolved(&b);
    let mut b = fixture();
    p64(&mut b, 0x508, 32);
    unresolved(&b);
    for index in [0, 32, u32::MAX] {
        let mut b = fixture();
        p32(&mut b, 0x500, index);
        unresolved(&b);
    }
    let mut b = fixture();
    b[0x541..0x560].fill(b'A');
    unresolved(&b);
    let mut b = fixture();
    b[0x541] = 0xff;
    unresolved(&b);
}

#[test]
fn malformed_linkedit_tables_and_dylib_commands_fail_without_panicking() {
    for (at, value) in [
        (344, u32::MAX),
        (348, u32::MAX),
        (352, u32::MAX),
        (356, u32::MAX),
        (416, u32::MAX),
        (420, u32::MAX),
        (448, 56),
        (448, 8),
        (440, 2),
    ] {
        let mut b = fixture();
        p32(&mut b, at, value);
        assert!(MachOImage::parse(&b).is_err(), "at={at} value={value}");
    }
    let mut b = fixture();
    b[464..496].fill(b'A');
    assert!(MachOImage::parse(&b).is_err());
}

#[test]
fn fat_slice_offsets_and_explicit_switch_overrides_remain_correct() {
    let b = fixture();
    let mut fat = vec![0; 0x1000 + b.len()];
    for (at, value) in [
        (0, 0xcafebabeu32),
        (4, 1),
        (8, 0x100000c),
        (16, 0x1000),
        (20, b.len() as u32),
    ] {
        fat[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }
    fat[0x1000..].copy_from_slice(&b);
    let image = MachOImage::parse(&fat).unwrap();
    assert!(image.is_universal);
    assert_eq!(image.objc.macho_imports[&0x1208].symbol, "_foo");
    let mut program = GhidraProgram::from_macho_image("override", &image);
    let hint = GhidraJumpTable::new(0x1208, vec![0x1210]);
    let proc = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut program,
        "override",
        0x1200,
        16,
        &[hint],
    );
    assert!(!proc.is_incomplete());
    assert!(proc
        .basic_blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .all(|i| !matches!(i.pcode.first(), Some(PcodeOp::MachOImportTailCall { .. }))));
}

#[test]
fn platform_stack_failure_and_exit_imports_are_nonreturning() {
    for (symbol, platform) in [
        (&b"\0___stack_chk_fail\0"[..], false),
        (&b"\0___stack_chk_fail\0"[..], true),
        (&b"\0_exit\0"[..], false),
        (&b"\0_exit\0"[..], true),
    ] {
        let mut b = fixture();
        b[0x540..0x560].fill(0);
        b[0x540..0x540 + symbol.len()].copy_from_slice(symbol);
        if platform {
            let library = b"/usr/lib/libSystem.B.dylib\0";
            b[464..496].fill(0);
            b[464..464 + library.len()].copy_from_slice(library);
        }
        p32(&mut b, 0x210, 0x97fffffc);
        p32(&mut b, 0x214, 0xd65f03c0);
        let (_, proc) = analyze(&b, 0x1210, 16);
        assert!(!proc.is_incomplete());
        assert_eq!(
            proc.basic_blocks
                .iter()
                .map(|bb| bb.instructions.len())
                .sum::<usize>(),
            if platform { 1 } else { 2 }
        );
        assert_eq!(
            matches!(
                proc.basic_blocks[0].instructions[0].pcode[0],
                PcodeOp::NoReturnCall { .. }
            ),
            platform
        );
    }
}

#[test]
fn function_start_metadata_is_bounded_uleb_and_preserves_slice_addresses() {
    let mut b = fixture();
    p32(&mut b, 16, 6);
    p32(&mut b, 20, 480);
    p32(&mut b, 496, 0x26);
    p32(&mut b, 500, 16);
    p32(&mut b, 504, 0x5c0);
    p32(&mut b, 508, 8);
    b[0x5c0..0x5c4].copy_from_slice(&[0x80, 4, 0x10, 0]);
    let image = MachOImage::parse(&b).unwrap();
    assert_eq!(
        image.segments[0].function_starts,
        std::collections::BTreeSet::from([0x1200, 0x1210])
    );
    let mut corrupt = b.clone();
    p32(&mut corrupt, 508, 1);
    assert!(MachOImage::parse(&corrupt).is_err());
    let mut corrupt = b.clone();
    corrupt[0x5c3] = 1;
    assert!(MachOImage::parse(&corrupt).is_err());
    let mut corrupt = b.clone();
    p32(&mut corrupt, 508, 11);
    corrupt[0x5c0..0x5ca].fill(0x80);
    assert!(MachOImage::parse(&corrupt).is_err());
    let mut corrupt = b;
    p32(&mut corrupt, 504, u32::MAX);
    assert!(MachOImage::parse(&corrupt).is_err());
}
