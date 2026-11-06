use ghidra_hopper_rs::{
    ghidra_port::GhidraProgram, ghidra_port::HopperHybridObjcAnalyzerRs, macho_objc::MachOImage,
    pcode_sleigh::PcodeOp,
};

#[test]
fn chained_got_symbol_ordinal_weakness_and_addend_must_agree_with_indirect_metadata() {
    for mode in 0..5 {
        let mut b = chained_fixture();
        match mode {
            0 => b[0x95c] = b'X',
            1 => p32(&mut b, 0x958, 2),
            2 => p32(&mut b, 0x958, 0x101),
            3 => p64(&mut b, 0x600, (1 << 63) | (1 << 24)),
            _ => p32(&mut b, 0x918, 1), // Compressed symbol pool is unsupported.
        }
        let image = MachOImage::parse(&b).unwrap();
        assert!(image.objc.macho_imports.is_empty(), "{mode}");
        assert!(image.objc.selector_stubs.is_empty(), "{mode}");
    }
    let mut b = chained_fixture();
    p64(&mut b, 0x600, (1 << 63) | 1); // ordinal outside the declared chained imports.
    assert!(MachOImage::parse(&b).is_err());
}

#[test]
fn upstream_chain_edit_invalidates_selector_even_when_its_own_pointer_is_unchanged() {
    let mut b = chained_fixture();
    b[0x93e..0x940].copy_from_slice(&0x3f8u16.to_le_bytes());
    p64(&mut b, 0x3f8, (2 << 51) | 0x480);
    let image = MachOImage::parse(&b).unwrap();
    assert_eq!(
        image.objc.selector_stubs.get(&0x1300).map(String::as_str),
        Some("echo:")
    );
    let mut p = GhidraProgram::from_macho_image("upstream", &image);
    p.segments[0].data[0x3fe] = 0; // Previous chain link no longer reaches the selref.
    let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut p,
        "upstream",
        0x12c0,
        16,
        &[],
    );
    assert!(matches!(
        f.basic_blocks[0].instructions[0].pcode[0],
        PcodeOp::Call { .. }
    ));
    b[0x3fe] = 0;
    assert!(MachOImage::parse(&b)
        .unwrap()
        .objc
        .selector_stubs
        .is_empty());
}

fn chained_fixture() -> Vec<u8> {
    let mut b = fixture();
    b.copy_within(264..576, 344);
    p32(&mut b, 16, 6);
    p32(&mut b, 20, 640);
    p32(&mut b, 32 + 4, 312);
    p32(&mut b, 32 + 64, 3);
    b[264..344].fill(0);
    section(&mut b, 264, b"__objc_methname", (0x1480, 0x480, 6), 2, 0);
    b[0x480..0x486].copy_from_slice(b"echo:\0");
    p64(&mut b, 0x400, 0x480);
    p32(&mut b, 656, 0x80000034);
    p32(&mut b, 660, 16);
    p32(&mut b, 664, 0x900);
    p32(&mut b, 668, 128);
    p32(&mut b, 0x904, 28);
    p32(&mut b, 0x908, 88);
    p32(&mut b, 0x90c, 92);
    p32(&mut b, 0x910, 1);
    p32(&mut b, 0x914, 1);
    p32(&mut b, 0x91c, 2);
    p32(&mut b, 0x920, 12);
    p32(&mut b, 0x924, 36);
    p32(&mut b, 0x928, 24);
    b[0x92c..0x92e].copy_from_slice(&4096u16.to_le_bytes());
    b[0x92e..0x930].copy_from_slice(&6u16.to_le_bytes());
    b[0x93c..0x93e].copy_from_slice(&1u16.to_le_bytes());
    b[0x93e..0x940].copy_from_slice(&0x400u16.to_le_bytes());
    p32(&mut b, 0x940, 24);
    b[0x944..0x946].copy_from_slice(&4096u16.to_le_bytes());
    b[0x946..0x948].copy_from_slice(&6u16.to_le_bytes());
    p64(&mut b, 0x948, 0x1000);
    b[0x954..0x956].copy_from_slice(&1u16.to_le_bytes());
    p32(&mut b, 0x958, 1);
    let symbol = b"_objc_msgSend\0";
    b[0x95c..0x95c + symbol.len()].copy_from_slice(symbol);
    p64(&mut b, 0x600, 1 << 63);
    p32(&mut b, 0x2c0, 0x94000010);
    p32(&mut b, 0x2c4, 0xd65f03c0);
    b
}
#[test]
fn chained_selector_is_named_only_with_full_msgsend_import_proof() {
    let image = MachOImage::parse(&chained_fixture()).unwrap();
    assert_eq!(
        image
            .objc
            .selector_references
            .get(&0x1400)
            .map(String::as_str),
        Some("echo:")
    );
    assert_eq!(
        image.objc.selector_stubs.get(&0x1300).map(String::as_str),
        Some("echo:")
    );
    let proof = &image.objc.selector_stub_proofs[&0x1300];
    assert!(proof.matches(0x1300, &image.segments));
    let mut p = GhidraProgram::from_macho_image("chain", &image);
    let mut analyzer = HopperHybridObjcAnalyzerRs::new();
    let f = analyzer.analyze_function_zero_ipc(&mut p, "chain", 0x12c0, 16, &[]);
    assert!(!f.is_incomplete());
    assert!(
        matches!(&f.basic_blocks[0].instructions[0].pcode[0],PcodeOp::CallObjC{selector,..} if selector=="echo:")
    );
    assert_eq!(analyzer.discovered_messages[0].receiver, "x0");
    assert!(p.external_methods.is_empty());
}
#[test]
fn absent_membership_bind_slots_unknown_formats_and_fake_tails_do_not_get_names() {
    for mode in 0..4 {
        let mut b = chained_fixture();
        match mode {
            0 => b[0x93e..0x940].copy_from_slice(&65535u16.to_le_bytes()),
            1 => p64(&mut b, 0x400, 1 << 63),
            2 => {
                b[0x92e..0x930].copy_from_slice(&12u16.to_le_bytes());
                p64(&mut b, 0x400, 0x1480);
            }
            _ => p32(&mut b, 0x310, 0xd65f03c0),
        }
        let image = MachOImage::parse(&b).unwrap();
        assert!(image.objc.selector_stubs.is_empty(), "{mode}");
    }
}
#[test]
fn changed_selector_reference_name_or_stub_cannot_reuse_loaded_objc_call_proof() {
    let image = MachOImage::parse(&chained_fixture()).unwrap();
    for (at, byte) in [(0x400, 0x81), (0x480, b'X'), (0x300, 0), (0x310, 0xff)] {
        let mut p = GhidraProgram::from_macho_image("changed", &image);
        p.segments[0].data[at] = byte;
        let mut analyzer = HopperHybridObjcAnalyzerRs::new();
        let f = analyzer.analyze_function_zero_ipc(&mut p, "changed", 0x12c0, 16, &[]);
        assert!(matches!(
            f.basic_blocks[0].instructions[0].pcode[0],
            PcodeOp::Call { .. }
        ));
        assert!(analyzer.discovered_messages.is_empty(), "{at:x}");
    }
}
fn p32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn p64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
fn section(b: &mut [u8], at: usize, name: &[u8], layout: (u64, u32, u64), flags: u32, first: u32) {
    let (addr, off, size) = layout;
    b[at..at + name.len()].copy_from_slice(name);
    p64(b, at + 32, addr);
    p64(b, at + 40, size);
    p32(b, at + 48, off);
    p32(b, at + 64, flags);
    p32(b, at + 68, first);
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0; 0x1000];
    p32(&mut b, 0, 0xfeedfacf);
    p32(&mut b, 4, 0x100000c);
    p32(&mut b, 12, 6);
    p32(&mut b, 16, 5);
    p32(&mut b, 20, 544);
    p32(&mut b, 24, 0x84);
    for (at, vm, off, len, prot, count, cmdsize) in [
        (32, 0x1000, 0, 0x500, 5, 2, 232),
        (264, 0x2000, 0x600, 0x100, 3, 1, 152),
    ] {
        p32(&mut b, at, 0x19);
        p32(&mut b, at + 4, cmdsize);
        p64(&mut b, at + 24, vm);
        p64(&mut b, at + 32, 0x1000);
        p64(&mut b, at + 40, off);
        p64(&mut b, at + 48, len);
        p32(&mut b, at + 56, prot);
        p32(&mut b, at + 60, prot);
        p32(&mut b, at + 64, count);
    }
    b[40..46].copy_from_slice(b"__TEXT");
    b[272..278].copy_from_slice(b"__DATA");
    section(
        &mut b,
        104,
        b"__objc_stubs",
        (0x1300, 0x300, 32),
        0x80000400,
        0,
    );
    section(&mut b, 184, b"__objc_selrefs", (0x1400, 0x400, 8), 0, 0);
    section(&mut b, 336, b"__got", (0x2000, 0x600, 8), 6, 1);
    p32(&mut b, 416, 2);
    p32(&mut b, 420, 24);
    p32(&mut b, 424, 0x800);
    p32(&mut b, 428, 1);
    p32(&mut b, 432, 0x840);
    p32(&mut b, 436, 32);
    p32(&mut b, 440, 11);
    p32(&mut b, 444, 80);
    p32(&mut b, 496, 0x880);
    p32(&mut b, 500, 2);
    p32(&mut b, 520, 0xc);
    p32(&mut b, 524, 56);
    p32(&mut b, 528, 24);
    let lib = b"/usr/lib/libobjc.A.dylib\0";
    b[544..544 + lib.len()].copy_from_slice(lib);
    for (n, w) in [0x90000001, 0xf9420021, 0xb0000010, 0xf9400210, 0xd61f0200]
        .iter()
        .enumerate()
    {
        p32(&mut b, 0x300 + n * 4, *w);
    }
    p64(&mut b, 0x400, 0x801000000000dead);
    p64(&mut b, 0x600, 0x801000000000beef);
    p32(&mut b, 0x800, 1);
    b[0x804] = 1;
    b[0x807] = 1;
    let name = b"\0_objc_msgSend\0";
    b[0x840..0x840 + name.len()].copy_from_slice(name);
    b
}
#[test]
fn objc_tail_resolves_import_but_never_invents_selector_or_runtime_pointer() {
    let image = MachOImage::parse(&fixture()).unwrap();
    let binding = &image.objc.macho_imports[&0x1310];
    assert_eq!(binding.symbol, "_objc_msgSend");
    assert_eq!(binding.selector_address, Some(0x1400));
    assert_eq!(binding.instructions.len(), 5);
    let mut p = GhidraProgram::from_macho_image("objc", &image);
    let mut analyzer = HopperHybridObjcAnalyzerRs::new();
    let f = analyzer.analyze_function_zero_ipc(&mut p, "objc", 0x1300, 16, &[]);
    assert!(!f.is_incomplete());
    assert!(analyzer.discovered_messages.is_empty());
    let text = f.render_four_modes(&p.segments[0]);
    assert!(text.contains("name and runtime value unresolved"));
    assert!(!text.contains("0000dead") && !text.contains("0000beef"));
    assert!(matches!(
        f.basic_blocks[0].instructions[4].pcode[0],
        PcodeOp::MachOImportTailCall {
            selector_address: Some(0x1400),
            ..
        }
    ));
}
#[test]
fn objc_suffix_adrp_uses_its_own_page_across_boundary() {
    let mut b = fixture();
    p64(&mut b, 32 + 24, 0x1cf8);
    p64(&mut b, 104 + 32, 0x1ff8);
    p64(&mut b, 184 + 32, 0x20f8);
    p64(&mut b, 264 + 24, 0x3000);
    p64(&mut b, 336 + 32, 0x3000);
    p32(&mut b, 0x300, 0xb0000001);
    p32(&mut b, 0x304, 0xf9407c21);
    let image = MachOImage::parse(&b).unwrap();
    let binding = &image.objc.macho_imports[&0x2008];
    assert_eq!(binding.pointer_address, 0x3000);
    assert_eq!(binding.selector_address, Some(0x20f8));
}
#[test]
fn malformed_prefix_selector_mapping_or_import_identity_stays_unresolved() {
    for (at, word) in [
        (0x300, 0x90000000),
        (0x304, 0xf9420421),
        (0x308, 0xb0000011),
        (184 + 48, 0x408),
        (336 + 68, 100),
    ] {
        let mut b = fixture();
        p32(&mut b, at, word);
        let image = MachOImage::parse(&b).unwrap();
        assert!(image.objc.macho_imports.is_empty(), "{at:x}");
    }
    let mut b = fixture();
    b[544..576].fill(0);
    b[544..552].copy_from_slice(b"private\0");
    assert!(MachOImage::parse(&b).unwrap().objc.macho_imports.is_empty());
    let mut b = fixture();
    b[0x841] = b'X';
    assert!(MachOImage::parse(&b).unwrap().objc.macho_imports.is_empty());
}
#[test]
fn incomplete_or_changed_objc_stub_cannot_reuse_full_prefix_proof() {
    for (entry, budget) in [(0x1308, 16), (0x1300, 4)] {
        let image = MachOImage::parse(&fixture()).unwrap();
        let mut p = GhidraProgram::from_macho_image("partial", &image);
        let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
            &mut p,
            "partial",
            entry,
            budget,
            &[],
        );
        assert!(f.is_incomplete());
    }
    let image = MachOImage::parse(&fixture()).unwrap();
    let mut p = GhidraProgram::from_macho_image("changed", &image);
    p32(&mut p.segments[0].data, 0x304, 0xd503201f);
    let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut p,
        "changed",
        0x1300,
        16,
        &[],
    );
    assert!(f.is_incomplete());
}
