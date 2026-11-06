use ghidra_hopper_rs::{
    cfg_tags::Procedure,
    macho_objc::MachOImage,
    pcode_sleigh::SwitchHintTable,
    segment::{ByteType, Segment},
};
use std::collections::BTreeMap;

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
fn put64(b: &mut [u8], off: usize, v: u64) {
    b[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn macho() -> Vec<u8> {
    let mut b = vec![0; 512];
    put32(&mut b, 0, 0xfeedfacf);
    put32(&mut b, 4, 0x0100000c);
    put32(&mut b, 16, 1);
    put32(&mut b, 20, 72 + 160);
    put32(&mut b, 32, 0x19);
    put32(&mut b, 36, 72 + 160);
    b[40..46].copy_from_slice(b"__TEXT");
    put64(&mut b, 56, 0x1000);
    put64(&mut b, 64, 512);
    put64(&mut b, 80, 512);
    put32(&mut b, 88, 5);
    put32(&mut b, 92, 5);
    put32(&mut b, 96, 2);
    b[104..119].copy_from_slice(b"__objc_methname");
    put64(&mut b, 136, 0x1140);
    put64(&mut b, 144, 4);
    put32(&mut b, 152, 320);
    b[184..198].copy_from_slice(b"__objc_selrefs");
    put64(&mut b, 216, 0x1150);
    put64(&mut b, 224, 16);
    put32(&mut b, 232, 336);
    b[320..324].copy_from_slice(b"a\0b\0");
    put64(&mut b, 336, 0x1142);
    put64(&mut b, 344, 0x1140);
    b
}

#[test]
fn selector_references_follow_pointers_instead_of_string_order() {
    let image = MachOImage::parse(&macho()).unwrap();
    assert_eq!(
        image
            .objc
            .selector_references
            .get(&0x1150)
            .map(String::as_str),
        Some("b")
    );
    assert_eq!(
        image
            .objc
            .selector_references
            .get(&0x1158)
            .map(String::as_str),
        Some("a")
    );
    assert!(image.objc.selector_stubs.is_empty());
}

#[test]
fn malformed_macho_commands_are_errors() {
    let mut b = macho();
    put32(&mut b, 36, 8); // LC_SEGMENT_64 cannot fit its header.
    assert!(MachOImage::parse(&b).is_err());
    let mut b = macho();
    put32(&mut b, 20, 8); // command exceeds sizeofcmds.
    assert!(MachOImage::parse(&b).is_err());
}

#[test]
fn malicious_elf_table_offset_returns_error_without_panicking() {
    let mut b = vec![0; 64];
    b[..6].copy_from_slice(b"\x7fELF\x02\x01");
    put64(&mut b, 32, u64::MAX - 8);
    b[54..56].copy_from_slice(&56u16.to_le_bytes());
    b[56..58].copy_from_slice(&1u16.to_le_bytes());
    assert!(MachOImage::parse(&b).is_err());
}

#[test]
fn elf_big_endian_is_rejected_instead_of_misdecoded() {
    let mut b = vec![0; 64];
    b[..6].copy_from_slice(b"\x7fELF\x02\x02");
    assert!(MachOImage::parse(&b).is_err());
}

#[test]
fn cfg_keeps_branch_target_after_an_early_return() {
    // cbz x0, +8; ret; ret -- both returns are reachable.
    let data = [0xb4000040u32, 0xd65f03c0, 0xd65f03c0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let mut seg = Segment::new("__TEXT", 0x1000, 0, data);
    let proc = Procedure::analyze_arm64(
        "f",
        0x1000,
        3,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    assert_eq!(proc.basic_blocks.len(), 3);
    assert_eq!(proc.basic_blocks[0].successors, vec![0x1008, 0x1004]);
    assert_eq!(seg.get_type_at_address(0x1008), ByteType::Code);
}

#[test]
fn segment_rejects_overflowing_lengths() {
    let mut seg = Segment::new("test", 0x1000, 0, vec![0; 8]);
    assert!(seg.read_bytes(0x1001, usize::MAX).is_none());
    assert!(!seg.set_type_at_address(0x1001, usize::MAX, ByteType::Code));
    assert!(seg.mark_range_as_undefined(0x1001, usize::MAX));
    assert_eq!(seg.file_offset_for_address(0x1001), Some(1));
    seg.file_offset = u64::MAX;
    assert_eq!(seg.file_offset_for_address(0x1001), None);
}

#[test]
fn objc_prefix_alone_does_not_prove_an_objc_call() {
    let mut b = macho();
    // Add a third section at 264, before the actual section data.
    put32(&mut b, 20, 312);
    put32(&mut b, 36, 312);
    put32(&mut b, 96, 3);
    // Move method/selref data beyond the enlarged load command area.
    b.copy_within(320..352, 400);
    put32(&mut b, 152, 400);
    put32(&mut b, 232, 416);
    put64(&mut b, 136, 0x1190);
    put64(&mut b, 216, 0x11a0);
    put64(&mut b, 416, 0x1192);
    put64(&mut b, 424, 0x1190);
    b[264..276].copy_from_slice(b"__objc_stubs");
    put64(&mut b, 296, 0x11c0);
    put64(&mut b, 304, 32);
    put32(&mut b, 312, 448);
    put32(&mut b, 448, 0x90000001); // adrp x1, current page
    put32(&mut b, 452, 0xf940d421); // ldr x1, [x1, #0x1a8] -> "a", not "b"
    let image = MachOImage::parse(&b).unwrap();
    assert_eq!(
        image
            .objc
            .selector_references
            .get(&0x11a8)
            .map(String::as_str),
        Some("a")
    );
    assert!(image.objc.selector_stubs.is_empty()); // no verified objc_msgSend tail
    put32(&mut b, 448, 0); // unrecognized instruction must not invent a selector
    assert!(MachOImage::parse(&b)
        .unwrap()
        .objc
        .selector_stubs
        .is_empty());
}

fn elf() -> Vec<u8> {
    let mut b = vec![0; 1024];
    b[..6].copy_from_slice(b"\x7fELF\x02\x01");
    b[18..20].copy_from_slice(&183u16.to_le_bytes());
    put64(&mut b, 32, 64);
    put64(&mut b, 40, 128);
    b[54..56].copy_from_slice(&56u16.to_le_bytes());
    b[56..58].copy_from_slice(&1u16.to_le_bytes());
    b[58..60].copy_from_slice(&64u16.to_le_bytes());
    b[60..62].copy_from_slice(&4u16.to_le_bytes());
    b[62..64].copy_from_slice(&1u16.to_le_bytes());
    put32(&mut b, 64, 1);
    put64(&mut b, 80, 0x1000);
    put64(&mut b, 96, 1024);
    let strings = b"\0.shstrtab\0.dynstr\0.dynsym\0";
    b[512..512 + strings.len()].copy_from_slice(strings);
    for (idx, name, off, len) in [
        (1, 1, 512, strings.len()),
        (2, 11, 600, 12),
        (3, 19, 640, 48),
    ] {
        let sh = 128 + idx * 64;
        put32(&mut b, sh, name);
        put32(&mut b, sh + 4, if idx == 3 { 11 } else { 3 });
        put64(&mut b, sh + 24, off);
        put64(&mut b, sh + 32, len as u64);
    }
    b[600..612].copy_from_slice(b"\0JNI_OnLoad\0");
    put32(&mut b, 664, 1);
    b[668] = 0x12;
    b[670] = 1;
    put64(&mut b, 672, 0x1000);
    b
}

#[test]
fn elf_exports_are_normal_symbols_and_never_objc_messages() {
    use ghidra_hopper_rs::{
        ghidra_port::GhidraProgram,
        pcode_sleigh::{decode_and_lift_arm64, PcodeOp},
    };
    let image = MachOImage::parse(&elf()).unwrap();
    assert!(image.objc.selector_stubs.is_empty());
    assert_eq!(
        image.objc.symbols.get(&0x1000).map(String::as_str),
        Some("JNI_OnLoad")
    );
    let prog = GhidraProgram::from_macho_image("test", &image);
    let insn = decode_and_lift_arm64(
        0x1000,
        0x94000000,
        prog.text_segment().unwrap(),
        &prog.objc_stub_selectors,
        &SwitchHintTable::default(),
    );
    assert!(matches!(insn.pcode[0], PcodeOp::Call { .. }));
}

#[test]
fn function_in_second_load_segment_is_actually_analyzed() {
    use ghidra_hopper_rs::ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs};
    let mut prog = GhidraProgram::new("test", Segment::new("__TEXT", 0x1000, 0, vec![0; 16]));
    prog.segments.push(Segment::new(
        "PT_LOAD_1",
        0x2000,
        16,
        0xd65f03c0u32.to_le_bytes().to_vec(),
    ));
    let proc =
        HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(&mut prog, "f", 0x2000, 4, &[]);
    assert_eq!(proc.basic_blocks[0].instructions[0].mnemonic, "ret");
    assert_eq!(
        prog.segments[0].get_type_at_address(0x1000),
        ByteType::Undefined
    );
    assert_eq!(
        prog.segments[1].get_type_at_address(0x2000),
        ByteType::Procedure
    );
}

#[test]
fn cfg_does_not_decode_dead_bytes_after_unconditional_branch() {
    let data = [0x14000002u32, 0, 0xd65f03c0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let mut seg = Segment::new("__TEXT", 0x1000, 0, data);
    let proc = Procedure::analyze_arm64(
        "f",
        0x1000,
        3,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    assert_eq!(proc.basic_blocks.len(), 2);
    assert_eq!(proc.basic_blocks[0].end_address, 0x1004);
    assert_eq!(seg.get_type_at_address(0x1004), ByteType::Undefined);
}

#[test]
fn oversized_swift_length_does_not_overflow() {
    use ghidra_hopper_rs::segment::demangle_symbol;
    assert!(demangle_symbol("$s99999999999999999999999999999bad").starts_with("swift::"));
}

#[test]
fn cbz_and_cbnz_have_opposite_pseudocode_conditions() {
    use ghidra_hopper_rs::pcode_sleigh::{
        decode_and_lift_arm64, decompile_instructions_to_pseudocode,
    };
    let seg = Segment::new("__TEXT", 0x1000, 0, vec![0; 16]);
    for (word, condition) in [(0xb4000040, "if (x0 == 0)"), (0xb5000040, "if (x0)")] {
        let insn = decode_and_lift_arm64(
            0x1000,
            word,
            &seg,
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        assert!(
            decompile_instructions_to_pseudocode("f", 0x1000, &[insn], &seg).contains(condition)
        );
    }
}

#[test]
fn cfg_backward_target_has_the_correct_block_start() {
    let data = [0xd65f03c0u32, 0x17ffffff]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let mut seg = Segment::new("__TEXT", 0x1000, 0, data);
    let proc = Procedure::analyze_arm64(
        "f",
        0x1004,
        3,
        &mut seg,
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    );
    assert_eq!(proc.basic_blocks[0].start_address, 0x1000);
    assert_eq!(proc.basic_blocks[1].start_address, 0x1004);
}

#[test]
fn parser_mutations_and_truncations_never_panic() {
    for original in [macho(), elf()] {
        for end in 0..original.len() {
            let _ = MachOImage::parse(&original[..end]);
        }
        for off in (0..original.len()).step_by(4) {
            for value in [0u32, u32::MAX, 0x80000000] {
                let mut b = original.clone();
                put32(&mut b, off, value);
                let _ = MachOImage::parse(&b);
            }
        }
    }
}
