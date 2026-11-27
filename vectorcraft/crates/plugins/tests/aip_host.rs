//! End-to-end tests for compiled C++ Illustrator `.aip` / `.bundle` SweetPea/PICA plug-ins
//! (`x86_64` PE32+ and `AArch64` Mach-O / Universal) through `vectorcraft-plugins`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use serde_json::json;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, LineCap, LineJoin, Node, NodeKind, StrokeLayer};
use vectorcraft_geom::{Anchor, AnchorKind, FillRule, PathData, Point, Rect, SubPath};
use vectorcraft_plugins::{Kind, Limits, Plugin, effect, objects, registry};

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Plugin>();
};

/// Builds a minimal, spec-valid 64-bit PE32+ (`x86_64`) `.aip` DLL exporting `export_name` with
/// `code` in `.text` (`RVA = 0x1000`) and optional binary `PiPL` payload in `.rsrc` (`RVA = 0x3000`).
fn build_test_pe64(export_name: &str, code: &[u8], pipl_payload: Option<&[u8]>) -> Vec<u8> {
    let mut img = vec![0u8; 0x4000];
    img[0..2].copy_from_slice(b"MZ");
    img[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());

    let pe = 0x80usize;
    img[pe..pe + 4].copy_from_slice(b"PE\0\0");
    img[pe + 4..pe + 6].copy_from_slice(&0x8664u16.to_le_bytes()); // AMD64
    let nsec: u16 = if pipl_payload.is_some() { 3 } else { 2 };
    img[pe + 6..pe + 8].copy_from_slice(&nsec.to_le_bytes());
    img[pe + 20..pe + 22].copy_from_slice(&0xF0u16.to_le_bytes());
    img[pe + 22..pe + 24].copy_from_slice(&0x2022u16.to_le_bytes());

    let opt = pe + 24;
    img[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // PE32+
    img[opt + 24..opt + 32].copy_from_slice(&0x0000_0001_8000_0000u64.to_le_bytes());
    img[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes());
    img[opt + 36..opt + 40].copy_from_slice(&0x0400u32.to_le_bytes());
    img[opt + 56..opt + 60].copy_from_slice(&0x5000u32.to_le_bytes());
    img[opt + 60..opt + 64].copy_from_slice(&0x0400u32.to_le_bytes());
    img[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());

    let dd = opt + 112;
    img[dd..dd + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    img[dd + 4..dd + 8].copy_from_slice(&0x200u32.to_le_bytes());
    if pipl_payload.is_some() {
        img[dd + 16..dd + 20].copy_from_slice(&0x3000u32.to_le_bytes());
        img[dd + 20..dd + 24].copy_from_slice(&0x800u32.to_le_bytes());
    }

    let sh = opt + 0xF0;
    let write_sec = |buf: &mut [u8], idx: usize, name: &[u8], vsize: u32, rva: u32, raw_sz: u32, raw_off: u32| {
        let o = sh + idx * 40;
        buf[o..o + name.len()].copy_from_slice(name);
        buf[o + 8..o + 12].copy_from_slice(&vsize.to_le_bytes());
        buf[o + 12..o + 16].copy_from_slice(&rva.to_le_bytes());
        buf[o + 16..o + 20].copy_from_slice(&raw_sz.to_le_bytes());
        buf[o + 20..o + 24].copy_from_slice(&raw_off.to_le_bytes());
    };
    write_sec(&mut img, 0, b".text", 0x1000, 0x1000, 0x0800, 0x0400);
    write_sec(&mut img, 1, b".edata", 0x1000, 0x2000, 0x0400, 0x0C00);
    if pipl_payload.is_some() {
        write_sec(&mut img, 2, b".rsrc", 0x1000, 0x3000, 0x0800, 0x1000);
    }

    img[0x0400..0x0400 + code.len()].copy_from_slice(code);

    let eo = 0x0C00usize;
    img[eo + 16..eo + 20].copy_from_slice(&1u32.to_le_bytes());
    img[eo + 20..eo + 24].copy_from_slice(&1u32.to_le_bytes());
    img[eo + 24..eo + 28].copy_from_slice(&1u32.to_le_bytes());
    img[eo + 28..eo + 32].copy_from_slice(&0x2040u32.to_le_bytes());
    img[eo + 32..eo + 36].copy_from_slice(&0x2044u32.to_le_bytes());
    img[eo + 36..eo + 40].copy_from_slice(&0x2048u32.to_le_bytes());
    img[eo + 0x40..eo + 0x44].copy_from_slice(&0x1000u32.to_le_bytes());
    img[eo + 0x44..eo + 0x48].copy_from_slice(&0x2050u32.to_le_bytes());
    img[eo + 0x48..eo + 0x4A].copy_from_slice(&0u16.to_le_bytes());
    let nb = export_name.as_bytes();
    img[eo + 0x50..eo + 0x50 + nb.len()].copy_from_slice(nb);

    if let Some(payload) = pipl_payload {
        let ro = 0x1000usize;
        img[ro + 14..ro + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 16..ro + 20].copy_from_slice(&10u32.to_le_bytes());
        img[ro + 20..ro + 24].copy_from_slice(&(0x8000_0020u32).to_le_bytes());
        img[ro + 0x20 + 14..ro + 0x20 + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 0x20 + 16..ro + 0x20 + 20].copy_from_slice(&16000u32.to_le_bytes());
        img[ro + 0x20 + 20..ro + 0x20 + 24].copy_from_slice(&(0x8000_0040u32).to_le_bytes());
        img[ro + 0x40 + 14..ro + 0x40 + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 0x40 + 16..ro + 0x40 + 20].copy_from_slice(&1033u32.to_le_bytes());
        img[ro + 0x40 + 20..ro + 0x40 + 24].copy_from_slice(&0x60u32.to_le_bytes());
        img[ro + 0x60..ro + 0x64].copy_from_slice(&0x3080u32.to_le_bytes());
        img[ro + 0x64..ro + 0x68].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        img[ro + 0x80..ro + 0x80 + payload.len()].copy_from_slice(payload);
    }

    img
}

fn build_le_pipl(kind_4cc: &[u8; 4], name: &str, category: &str, match_name: &str, ep_win64: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let push_prop = |buf: &mut Vec<u8>, key: &[u8; 4], data: &[u8]| {
        buf.extend_from_slice(b"MIB8");
        buf.extend_from_slice(&[key[3], key[2], key[1], key[0]]);
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(data);
        while !buf.len().is_multiple_of(4) {
            buf.push(0);
        }
    };
    push_prop(&mut out, b"kind", &[kind_4cc[3], kind_4cc[2], kind_4cc[1], kind_4cc[0]]);
    let mut pname = vec![name.len() as u8];
    pname.extend_from_slice(name.as_bytes());
    push_prop(&mut out, b"name", &pname);
    let mut pcat = vec![category.len() as u8];
    pcat.extend_from_slice(category.as_bytes());
    push_prop(&mut out, b"catg", &pcat);
    let mut pmna = vec![match_name.len() as u8];
    pmna.extend_from_slice(match_name.as_bytes());
    push_prop(&mut out, b"eMNA", &pmna);
    let mut pep = ep_win64.as_bytes().to_vec();
    pep.push(0);
    push_prop(&mut out, b"8664", &pep);
    out
}

/// Builds a minimal, spec-valid 64-bit `AArch64` (`arm64`) Mach-O `.aip` / `.bundle` exporting
/// `_<export_name>` at `0x1_0000_0400`, optionally wrapped in a Universal Fat (`0xCAFEBABE`) header.
fn build_test_macho_arm64(export_name: &str, instructions: &[u32], wrap_universal: bool) -> Vec<u8> {
    let mut mo = vec![0u8; 0x1000];
    mo[0..4].copy_from_slice(&0xFEED_FACFu32.to_le_bytes());
    mo[4..8].copy_from_slice(&0x0100_000Cu32.to_le_bytes()); // CPU_TYPE_ARM64
    mo[12..16].copy_from_slice(&0x08u32.to_le_bytes()); // MH_BUNDLE
    mo[16..20].copy_from_slice(&2u32.to_le_bytes());
    mo[20..24].copy_from_slice(&((152 + 24) as u32).to_le_bytes());

    let lc = 32usize;
    mo[lc..lc + 4].copy_from_slice(&0x19u32.to_le_bytes());
    mo[lc + 4..lc + 8].copy_from_slice(&152u32.to_le_bytes());
    mo[lc + 8..lc + 14].copy_from_slice(b"__TEXT");
    mo[lc + 24..lc + 32].copy_from_slice(&0x0000_0001_0000_0000u64.to_le_bytes());
    mo[lc + 32..lc + 40].copy_from_slice(&0x1000u64.to_le_bytes());
    mo[lc + 40..lc + 48].copy_from_slice(&0u64.to_le_bytes());
    mo[lc + 48..lc + 56].copy_from_slice(&0x0800u64.to_le_bytes());
    mo[lc + 64..lc + 68].copy_from_slice(&1u32.to_le_bytes());

    let sec = lc + 72;
    mo[sec..sec + 6].copy_from_slice(b"__text");
    mo[sec + 16..sec + 22].copy_from_slice(b"__TEXT");
    mo[sec + 32..sec + 40].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());
    mo[sec + 40..sec + 48].copy_from_slice(&((instructions.len() * 4) as u64).to_le_bytes());
    mo[sec + 48..sec + 52].copy_from_slice(&0x0400u32.to_le_bytes());

    let sym_lc = lc + 152;
    mo[sym_lc..sym_lc + 4].copy_from_slice(&0x02u32.to_le_bytes());
    mo[sym_lc + 4..sym_lc + 8].copy_from_slice(&24u32.to_le_bytes());
    mo[sym_lc + 8..sym_lc + 12].copy_from_slice(&0x0800u32.to_le_bytes());
    mo[sym_lc + 12..sym_lc + 16].copy_from_slice(&1u32.to_le_bytes());
    mo[sym_lc + 16..sym_lc + 20].copy_from_slice(&0x0820u32.to_le_bytes());
    mo[sym_lc + 20..sym_lc + 24].copy_from_slice(&0x0100u32.to_le_bytes());

    for (i, &insn) in instructions.iter().enumerate() {
        let o = 0x0400 + i * 4;
        mo[o..o + 4].copy_from_slice(&insn.to_le_bytes());
    }

    let sym_str = format!("_{export_name}");
    mo[0x0820 + 1..0x0820 + 1 + sym_str.len()].copy_from_slice(sym_str.as_bytes());
    mo[0x0800..0x0804].copy_from_slice(&1u32.to_le_bytes());
    mo[0x0804] = 0x0F;
    mo[0x0805] = 1;
    mo[0x0808..0x0810].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());

    if !wrap_universal {
        return mo;
    }
    let mut fat = vec![0u8; 0x100 + mo.len()];
    fat[0..4].copy_from_slice(&0xCAFE_BABEu32.to_be_bytes());
    fat[4..8].copy_from_slice(&1u32.to_be_bytes());
    fat[8..12].copy_from_slice(&0x0100_000Cu32.to_be_bytes());
    fat[16..20].copy_from_slice(&0x100u32.to_be_bytes());
    fat[20..24].copy_from_slice(&(mo.len() as u32).to_be_bytes());
    fat[24..28].copy_from_slice(&12u32.to_be_bytes());
    fat[0x100..].copy_from_slice(&mo);
    fat
}

/// Assembles a clean-room `AArch64` `.aip` that:
/// - Optionally registers an `AILiveEffectSuite` live effect named `"Arm64 Curve Offset"` during `"Startup"`;
/// - On `"Go"` / `"Go Live Effect"`, acquires `"AI Matching Art Suite"` and `"AI Path Suite"`,
///   iterates every selected `AIArtHandle` and every `AIPathSegment`, and translates `(p, h_in, h_out)`
///   by `(+12.5, -4.25)`!
fn build_arm64_bezier_aip(dx: f64, dy: f64, register_live_effect: bool, universal: bool) -> Vec<u8> {
    let encode_adr = |rd: u32, from_idx: usize, to_idx: usize| -> u32 {
        let byte_delta = ((to_idx as isize - from_idx as isize) * 4) as i32;
        let immlo = (byte_delta as u32) & 3;
        let immhi = ((byte_delta >> 2) as u32) & 0x7FFFF;
        0x1000_0000 | (immlo << 29) | (immhi << 5) | rd
    };
    let encode_b_cond = |cond: u32, from_idx: usize, to_idx: usize| -> u32 {
        let word_delta = (to_idx as isize - from_idx as isize) as i32;
        0x5400_0000 | (((word_delta as u32) & 0x7FFFF) << 5) | cond
    };
    let encode_cbz = |rt: u32, from_idx: usize, to_idx: usize| -> u32 {
        let word_delta = (to_idx as isize - from_idx as isize) as i32;
        0xB400_0000 | (((word_delta as u32) & 0x7FFFF) << 5) | rt
    };
    let encode_cbnz = |rt: u32, from_idx: usize, to_idx: usize| -> u32 {
        let word_delta = (to_idx as isize - from_idx as isize) as i32;
        0xB500_0000 | (((word_delta as u32) & 0x7FFFF) << 5) | rt
    };
    let encode_b = |from_idx: usize, to_idx: usize| -> u32 {
        let word_delta = (to_idx as isize - from_idx as isize) as i32;
        0x1400_0000 | ((word_delta as u32) & 0x03FF_FFFF)
    };

    // Stack frame: 192 bytes (`0xC0`), 16-byte aligned:
    // [sp + 0x00..0x5F]: x29,x30, x19..x28
    // [sp + 0x60]: suite_ptr (u64)
    // [sp + 0x68]: matches_hdl (u64)
    // [sp + 0x70]: match_count (i32)
    // [sp + 0x74]: seg_count (i32)
    // [sp + 0x78..0xAF]: AIPathSegment (56 bytes)
    // [sp + 0x80..0x9F]: scratch AILiveEffectData on Startup
    let mut insns: Vec<u32> = vec![
        0xD103_03FF, // sub sp, sp, #192
        0xA900_7BFD, // stp x29, x30, [sp, #0]
        0xA901_53F3, // stp x19, x20, [sp, #16]
        0xA902_5BF5, // stp x21, x22, [sp, #32]
        0xA903_63F7, // stp x23, x24, [sp, #48]
        0xA904_6BF9, // stp x25, x26, [sp, #64]
        0xA905_73FB, // stp x27, x28, [sp, #80]
        0x9100_03FD, // mov x29, sp
        0xAA01_03F4, // mov x20, x1 (selector*)
        0xAA02_03F3, // mov x19, x2 (message*)
        0xF940_0E75, // ldr x21, [x19, #0x18] (SPBasicSuite*)
        0xF940_1A76, // ldr x22, [x19, #0x30] (art handle: 0 on Startup/Shutdown, !=0 on Go)
    ];

    let mut patch_startup_go = None;
    let mut patch_startup_not_a = None;
    let mut patch_adr_live_suite = None;
    let mut patch_adr_live_title = None;
    let mut patch_startup_done = None;

    if register_live_effect {
        let idx_cbnz = insns.len();
        insns.push(0); // cbnz x22, go_entry
        patch_startup_go = Some(idx_cbnz);

        // Check `selector[2] == 'a'` ("Startup" vs "Shutdown")
        insns.push(0x3940_0A88); // ldrb w8, [x20, #2]
        insns.push(0x7101_851F); // cmp w8, #0x61 ('a')
        let idx_bne = insns.len();
        insns.push(0); // b.ne done
        patch_startup_not_a = Some(idx_bne);

        // AcquireSuite("AI Live Effect Suite", 1, sp + 0x60)
        let idx_adr_s = insns.len();
        insns.push(0); // adr x0, str_live_effect_suite
        patch_adr_live_suite = Some(idx_adr_s);
        insns.push(0xD280_0021); // mov x1, #1
        insns.push(0x9101_83A2); // add x2, x29, #0x60
        insns.push(0xF940_02A8); // ldr x8, [x21, #0]
        insns.push(0xD63F_0100); // blr x8

        insns.push(0xF940_33B6); // ldr x22, [x29, #0x60] (AILiveEffectSuite*)
        // Build AILiveEffectData at [x29 + 0x78]: +0x00 = self(0), +0x08 = title, +0x10 = title
        let idx_adr_t = insns.len();
        insns.push(0); // adr x9, str_live_title
        patch_adr_live_title = Some(idx_adr_t);
        insns.push(0xF900_3FA9); // str x9, [x29, #0x78] (+0x00 fallback)
        insns.push(0xF900_43A9); // str x9, [x29, #0x80] (+0x08 name)
        insns.push(0xF900_47A9); // str x9, [x29, #0x88] (+0x10 title)
        insns.push(0x9101_E3A0); // add x0, x29, #0x78 (data*)
        insns.push(0x9101_83A1); // add x1, x29, #0x60 (&effect_handle)
        insns.push(0xD280_0002); // mov x2, #0
        insns.push(0xD280_0003); // mov x3, #0
        insns.push(0xF940_02C8); // ldr x8, [x22, #0] (AddLiveEffect)
        insns.push(0xD63F_0100); // blr x8
        let idx_b_done = insns.len();
        insns.push(0); // b done
        patch_startup_done = Some(idx_b_done);
    }

    let go_entry = insns.len();
    let idx_cbz_go = insns.len();
    insns.push(0); // cbz x22, done

    // 1. AcquireSuite("AI Matching Art Suite", 1, x29 + 0x60) -> x22
    let idx_adr_match = insns.len();
    insns.push(0); // adr x0, str_matching_suite
    insns.push(0xD280_0021); // mov x1, #1
    insns.push(0x9101_83A2); // add x2, x29, #0x60
    insns.push(0xF940_02A8); // ldr x8, [x21, #0]
    insns.push(0xD63F_0100); // blr x8
    insns.push(0xF940_33B6); // ldr x22, [x29, #0x60] (AIMatchingArtSuite*)

    // 2. AcquireSuite("AI Path Suite", 1, x29 + 0x60) -> x23
    let idx_adr_path = insns.len();
    insns.push(0); // adr x0, str_path_suite
    insns.push(0xD280_0021); // mov x1, #1
    insns.push(0x9101_83A2); // add x2, x29, #0x60
    insns.push(0xF940_02A8); // ldr x8, [x21, #0]
    insns.push(0xD63F_0100); // blr x8
    insns.push(0xF940_33B7); // ldr x23, [x29, #0x60] (AIPathSuite*)

    // 3. AIMatchingArtSuite::GetSelectedArt(&matches_hdl=x29+0x68, &match_count=x29+0x70)
    insns.push(0x9101_A3A0); // add x0, x29, #0x68
    insns.push(0x9101_C3A1); // add x1, x29, #0x70
    insns.push(0xF940_02C8); // ldr x8, [x22, #0]
    insns.push(0xD63F_0100); // blr x8

    insns.push(0xF940_37B8); // ldr x24, [x29, #0x68] (Handle)
    insns.push(0xF940_0318); // ldr x24, [x24, #0]    (AIArtHandle* array)
    insns.push(0xB940_73B9); // ldr w25, [x29, #0x70] (match_count)
    insns.push(0xD280_001A); // mov x26, #0           (art_i = 0)

    let art_loop = insns.len();
    insns.push(0x6B19_035F); // cmp w26, w25
    let idx_art_end = insns.len();
    insns.push(0); // b.ge done (cond = 0xA = GE)

    // x27 = arts[x26]
    insns.push(0xF87A_7B1B); // ldr x27, [x24, x26, lsl #3]
    // GetPathSegmentCount(art=x27, &seg_count=x29+0x74)
    insns.push(0xAA1B_03E0); // mov x0, x27
    insns.push(0x9101_D3A1); // add x1, x29, #0x74
    insns.push(0xF940_02E8); // ldr x8, [x23, #0]
    insns.push(0xD63F_0100); // blr x8

    insns.push(0xD280_001C); // mov x28, #0 (seg_i = 0)
    let seg_loop = insns.len();
    insns.push(0xB940_77A9); // ldr w9, [x29, #0x74] (seg_count)
    insns.push(0x6B09_039F); // cmp w28, w9
    let idx_seg_end = insns.len();
    insns.push(0); // b.ge next_art

    // GetPathSegments(art=x27, seg=w28, count=1, buf=x29+0x78)
    insns.push(0xAA1B_03E0); // mov x0, x27
    insns.push(0xAA1C_03E1); // mov x1, x28
    insns.push(0xD280_0022); // mov x2, #1
    insns.push(0x9101_E3A3); // add x3, x29, #0x78
    insns.push(0xF940_0AE8); // ldr x8, [x23, #0x10] (GetPathSegments)
    insns.push(0xD63F_0100); // blr x8

    // Load dx (d6) and dy (d7) from literal pool, translate p, h_in, h_out!
    let idx_adr_consts = insns.len();
    insns.push(0); // adr x9, consts_dx_dy
    insns.push(0xFD40_0126); // ldr d6, [x9, #0] (dx)
    insns.push(0xFD40_0527); // ldr d7, [x9, #8] (dy)

    // p.x (+0x78), p.y (+0x80), h_in.x (+0x88), h_in.y (+0x90), h_out.x (+0x98), h_out.y (+0xA0)
    for (x_imm8, y_imm8) in [(15u32, 16u32), (17u32, 18u32), (19u32, 20u32)] {
        insns.push(0xFD40_03A0 | (x_imm8 << 10)); // ldr d0, [x29, #x_off]
        insns.push(0xFD40_03A1 | (y_imm8 << 10)); // ldr d1, [x29, #y_off]
        insns.push(0x1E66_2800); // fadd d0, d0, d6
        insns.push(0x1E67_2821); // fadd d1, d1, d7
        insns.push(0xFD00_03A0 | (x_imm8 << 10)); // str d0, [x29, #x_off]
        insns.push(0xFD00_03A1 | (y_imm8 << 10)); // str d1, [x29, #y_off]
    }

    // SetPathSegments(art=x27, seg=w28, count=1, buf=x29+0x78)
    insns.push(0xAA1B_03E0); // mov x0, x27
    insns.push(0xAA1C_03E1); // mov x1, x28
    insns.push(0xD280_0022); // mov x2, #1
    insns.push(0x9101_E3A3); // add x3, x29, #0x78
    insns.push(0xF940_0EE8); // ldr x8, [x23, #0x18] (SetPathSegments)
    insns.push(0xD63F_0100); // blr x8

    insns.push(0x9100_079C); // add x28, x28, #1
    let idx_b_seg = insns.len();
    insns.push(encode_b(idx_b_seg, seg_loop));

    let next_art = insns.len();
    insns.push(0x9100_075A); // add x26, x26, #1
    let idx_b_art = insns.len();
    insns.push(encode_b(idx_b_art, art_loop));

    let done = insns.len();
    insns.push(0xD280_0000); // mov x0, #0
    insns.push(0xA945_73FB); // ldp x27, x28, [sp, #80]
    insns.push(0xA944_6BF9); // ldp x25, x26, [sp, #64]
    insns.push(0xA943_63F7); // ldp x23, x24, [sp, #48]
    insns.push(0xA942_5BF5); // ldp x21, x22, [sp, #32]
    insns.push(0xA941_53F3); // ldp x19, x20, [sp, #16]
    insns.push(0xA940_7BFD); // ldp x29, x30, [sp, #0]
    insns.push(0x9103_03FF); // add sp, sp, #192
    insns.push(0xD65F_03C0); // ret

    // Align literal pool to 8 bytes (2 words)
    if !insns.len().is_multiple_of(2) {
        insns.push(0xD503_201F); // nop
    }
    let consts_idx = insns.len();
    for val in [dx, dy] {
        let b = val.to_le_bytes();
        insns.push(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        insns.push(u32::from_le_bytes([b[4], b[5], b[6], b[7]]));
    }

    let push_cstr = |buf: &mut Vec<u32>, s: &[u8]| -> usize {
        let idx = buf.len();
        let mut padded = s.to_vec();
        padded.push(0);
        while !padded.len().is_multiple_of(4) {
            padded.push(0);
        }
        for c in padded.as_chunks::<4>().0 {
            buf.push(u32::from_le_bytes(*c));
        }
        idx
    };

    let str_match_idx = push_cstr(&mut insns, b"AI Matching Art Suite");
    let str_path_idx = push_cstr(&mut insns, b"AI Path Suite");
    let str_live_idx = push_cstr(&mut insns, b"AI Live Effect Suite");
    let str_title_idx = push_cstr(&mut insns, b"Arm64 Curve Offset");

    // Patch branches & ADR instructions
    insns[idx_cbz_go] = encode_cbz(22, idx_cbz_go, done);
    insns[idx_adr_match] = encode_adr(0, idx_adr_match, str_match_idx);
    insns[idx_adr_path] = encode_adr(0, idx_adr_path, str_path_idx);
    insns[idx_art_end] = encode_b_cond(0x0A, idx_art_end, done);
    insns[idx_seg_end] = encode_b_cond(0x0A, idx_seg_end, next_art);
    insns[idx_adr_consts] = encode_adr(9, idx_adr_consts, consts_idx);

    if let Some(i) = patch_startup_go {
        insns[i] = encode_cbnz(22, i, go_entry);
    }
    if let Some(i) = patch_startup_not_a {
        insns[i] = encode_b_cond(0x01, i, done); // NE = 1
    }
    if let Some(i) = patch_adr_live_suite {
        insns[i] = encode_adr(0, i, str_live_idx);
    }
    if let Some(i) = patch_adr_live_title {
        insns[i] = encode_adr(9, i, str_title_idx);
    }
    if let Some(i) = patch_startup_done {
        insns[i] = encode_b(i, done);
    }

    build_test_macho_arm64("PluginMain", &insns, universal)
}

/// Assembles a clean-room `x86_64` PE32+ `.aip` (Microsoft x64 ABI) acquiring `"AI Matching Art Suite"`
/// and `"AI Path Suite"` and translating `(p, h_in, h_out)` of every segment of every selected path
/// by `(dx, dy)`.
fn build_x86_64_bezier_aip(dx: f64, dy: f64) -> Vec<u8> {
    let mut code: Vec<u8> = Vec::new();
    let mut patches_rel32: Vec<(usize, usize)> = Vec::new(); // (disp32_pos, target_kind)

    // Prologue: 7 pushes -> rsp % 16 == 0, then sub rsp, 0x90 -> rsp % 16 == 0!
    code.extend_from_slice(&[
        0x53, // push rbx
        0x56, // push rsi
        0x57, // push rdi
        0x41, 0x54, // push r12
        0x41, 0x55, // push r13
        0x41, 0x56, // push r14
        0x41, 0x57, // push r15
        0x48, 0x81, 0xEC, 0x90, 0x00, 0x00, 0x00, // sub rsp, 0x90
        0x4C, 0x89, 0xC3, // mov rbx, r8 (message*)
        0x48, 0x8B, 0x43, 0x30, // mov rax, [rbx + 0x30] (art handle)
        0x48, 0x85, 0xC0, // test rax, rax
    ]);
    // jz done (0F 84 disp32)
    code.extend_from_slice(&[0x0F, 0x84]);
    let jz_done_pos = code.len();
    code.extend_from_slice(&[0; 4]);

    // rsi = [rbx + 0x18] (SPBasicSuite*)
    code.extend_from_slice(&[0x48, 0x8B, 0x73, 0x18]);

    // AcquireSuite("AI Matching Art Suite", 1, &rsp+0x28)
    code.extend_from_slice(&[0x48, 0x8D, 0x0D]); // lea rcx, [rip + disp32]
    patches_rel32.push((code.len(), 0));
    code.extend_from_slice(&[0; 4]);
    code.extend_from_slice(&[
        0xBA, 0x01, 0x00, 0x00, 0x00, // mov edx, 1
        0x4C, 0x8D, 0x44, 0x24, 0x28, // lea r8, [rsp + 0x28]
        0xFF, 0x16, // call [rsi]
        0x4C, 0x8B, 0x64, 0x24, 0x28, // mov r12, [rsp + 0x28] (AIMatchingArtSuite*)
    ]);

    // AcquireSuite("AI Path Suite", 1, &rsp+0x28)
    code.extend_from_slice(&[0x48, 0x8D, 0x0D]); // lea rcx, [rip + disp32]
    patches_rel32.push((code.len(), 1));
    code.extend_from_slice(&[0; 4]);
    code.extend_from_slice(&[
        0xBA, 0x01, 0x00, 0x00, 0x00, // mov edx, 1
        0x4C, 0x8D, 0x44, 0x24, 0x28, // lea r8, [rsp + 0x28]
        0xFF, 0x16, // call [rsi]
        0x4C, 0x8B, 0x6C, 0x24, 0x28, // mov r13, [rsp + 0x28] (AIPathSuite*)
    ]);

    // GetSelectedArt(&rsp+0x30, &rsp+0x38)
    code.extend_from_slice(&[
        0x48, 0x8D, 0x4C, 0x24, 0x30, // lea rcx, [rsp + 0x30]
        0x48, 0x8D, 0x54, 0x24, 0x38, // lea rdx, [rsp + 0x38]
        0x41, 0xFF, 0x14, 0x24, // call [r12]
        0x4C, 0x8B, 0x74, 0x24, 0x30, // mov r14, [rsp + 0x30] (Handle)
        0x4D, 0x8B, 0x36, // mov r14, [r14]        (AIArtHandle* array)
        0x8B, 0x7C, 0x24, 0x38, // mov edi, [rsp + 0x38] (match_count)
        0x45, 0x31, 0xFF, // xor r15d, r15d        (art_i = 0)
    ]);

    let art_loop = code.len();
    code.extend_from_slice(&[0x41, 0x39, 0xFF]); // cmp r15d, edi
    code.extend_from_slice(&[0x0F, 0x8D]); // jge done
    let jge_done_pos = code.len();
    code.extend_from_slice(&[0; 4]);

    // rbx = [r14 + r15 * 8] (art handle)
    code.extend_from_slice(&[
        0x4B, 0x8B, 0x1C, 0xFE, // mov rbx, [r14 + r15 * 8]
        0x48, 0x89, 0xD9, // mov rcx, rbx
        0x48, 0x8D, 0x54, 0x24, 0x3C, // lea rdx, [rsp + 0x3C] (&seg_count)
        0x41, 0xFF, 0x55, 0x00, // call [r13 + 0] (GetPathSegmentCount)
        0x31, 0xF6, // xor esi, esi (seg_i = 0)
    ]);

    let seg_loop = code.len();
    code.extend_from_slice(&[
        0x3B, 0x74, 0x24, 0x3C, // cmp esi, [rsp + 0x3C]
        0x0F, 0x8D, // jge next_art
    ]);
    let jge_next_art_pos = code.len();
    code.extend_from_slice(&[0; 4]);

    // GetPathSegments(rcx=rbx, edx=esi, r8d=1, r9=rsp+0x40) -> [r13 + 0x10]
    code.extend_from_slice(&[
        0x48, 0x89, 0xD9, // mov rcx, rbx
        0x89, 0xF2, // mov edx, esi
        0x41, 0xB8, 0x01, 0x00, 0x00, 0x00, // mov r8d, 1
        0x4C, 0x8D, 0x4C, 0x24, 0x40, // lea r9, [rsp + 0x40]
        0x41, 0xFF, 0x55, 0x10, // call [r13 + 0x10]
    ]);

    // Load dx -> xmm1, dy -> xmm2
    code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x0D]); // movsd xmm1, [rip + disp32]
    patches_rel32.push((code.len(), 2));
    code.extend_from_slice(&[0; 4]);
    code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x15]); // movsd xmm2, [rip + disp32]
    patches_rel32.push((code.len(), 3));
    code.extend_from_slice(&[0; 4]);

    // Translate (p, h_in, h_out) at [rsp + 0x40..0x70]
    for (x_off, y_off) in [(0x40u8, 0x48u8), (0x50u8, 0x58u8), (0x60u8, 0x68u8)] {
        code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x44, 0x24, x_off]); // movsd xmm0, [rsp + x_off]
        code.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC1]); // addsd xmm0, xmm1
        code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x44, 0x24, x_off]); // movsd [rsp + x_off], xmm0

        code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x44, 0x24, y_off]); // movsd xmm0, [rsp + y_off]
        code.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC2]); // addsd xmm0, xmm2
        code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x44, 0x24, y_off]); // movsd [rsp + y_off], xmm0
    }

    // SetPathSegments(rcx=rbx, edx=esi, r8d=1, r9=rsp+0x40) -> [r13 + 0x18]
    code.extend_from_slice(&[
        0x48, 0x89, 0xD9, // mov rcx, rbx
        0x89, 0xF2, // mov edx, esi
        0x41, 0xB8, 0x01, 0x00, 0x00, 0x00, // mov r8d, 1
        0x4C, 0x8D, 0x4C, 0x24, 0x40, // lea r9, [rsp + 0x40]
        0x41, 0xFF, 0x55, 0x18, // call [r13 + 0x18]
        0xFF, 0xC6, // inc esi
    ]);
    // jmp seg_loop (E9 disp32)
    code.push(0xE9);
    let jmp_seg_pos = code.len();
    code.extend_from_slice(&((seg_loop as isize - (jmp_seg_pos + 4) as isize) as i32).to_le_bytes());

    let next_art = code.len();
    code.extend_from_slice(&[0x41, 0xFF, 0xC7]); // inc r15d
    code.push(0xE9);
    let jmp_art_pos = code.len();
    code.extend_from_slice(&((art_loop as isize - (jmp_art_pos + 4) as isize) as i32).to_le_bytes());

    let done = code.len();
    code.extend_from_slice(&[
        0x31, 0xC0, // xor eax, eax
        0x48, 0x81, 0xC4, 0x90, 0x00, 0x00, 0x00, // add rsp, 0x90
        0x41, 0x5F, // pop r15
        0x41, 0x5E, // pop r14
        0x41, 0x5D, // pop r13
        0x41, 0x5C, // pop r12
        0x5F, // pop rdi
        0x5E, // pop rsi
        0x5B, // pop rbx
        0xC3, // ret
    ]);

    while !code.len().is_multiple_of(8) {
        code.push(0x90);
    }
    let dx_pos = code.len();
    code.extend_from_slice(&dx.to_le_bytes());
    let dy_pos = code.len();
    code.extend_from_slice(&dy.to_le_bytes());
    let str_match_pos = code.len();
    code.extend_from_slice(b"AI Matching Art Suite\0");
    let str_path_pos = code.len();
    code.extend_from_slice(b"AI Path Suite\0");

    let write_rel32 = |buf: &mut [u8], pos: usize, target: usize| {
        let rel = (target as isize - (pos + 4) as isize) as i32;
        buf[pos..pos + 4].copy_from_slice(&rel.to_le_bytes());
    };
    write_rel32(&mut code, jz_done_pos, done);
    write_rel32(&mut code, jge_done_pos, done);
    write_rel32(&mut code, jge_next_art_pos, next_art);
    for (pos, kind) in patches_rel32 {
        let target = match kind {
            0 => str_match_pos,
            1 => str_path_pos,
            2 => dx_pos,
            _ => dy_pos,
        };
        write_rel32(&mut code, pos, target);
    }

    let pipl = build_le_pipl(b"AIFl", "Translate Bezier C++", "VectorCraft Native", "com.vectorcraft.translate_bezier", "PluginMain");
    build_test_pe64("PluginMain", &code, Some(&pipl))
}

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn sample_curved_document() -> (Document, vectorcraft_doc::NodeId) {
    let mut doc = Document::new(400.0, 300.0);
    let id = doc.alloc_id();
    let anchors = vec![
        Anchor { p: Point::new(20.0, 30.0), h_in: Point::new(10.0, 30.0), h_out: Point::new(35.0, 30.0), kind: AnchorKind::Smooth },
        Anchor { p: Point::new(80.0, 90.0), h_in: Point::new(80.0, 65.0), h_out: Point::new(80.0, 115.0), kind: AnchorKind::Smooth },
        Anchor { p: Point::new(20.0, 120.0), h_in: Point::new(20.0, 120.0), h_out: Point::new(20.0, 120.0), kind: AnchorKind::Corner },
    ];
    let path = PathData::single(SubPath { anchors, closed: true });
    let mut stroke = StrokeLayer::new(Paint::solid(Color::rgb(0.1, 0.4, 0.9)), 3.5);
    stroke.join = LineJoin::Round;
    stroke.cap = LineCap::Square;
    stroke.miter_limit = 8.0;
    let ap = Appearance {
        items: vec![AppearanceItem::Fill(FillLayer::new(Paint::solid(Color::rgb(0.85, 0.25, 0.15)))), AppearanceItem::Stroke(stroke)],
        ..Appearance::default()
    };
    let mut node = Node::path(id, path, ap);
    node.opacity = 0.75;
    node.name = Some("Curved Badge".into());
    std::sync::Arc::make_mut(&mut doc.layers[0]).children_mut().expect("layer children").push(std::sync::Arc::new(node));
    (doc, id)
}

#[test]
fn clean_room_x86_64_pe64_aip_transforms_bezier_anchors_and_preserves_appearance() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    vectorcraft_plugins::safe_mode::set_active(None);

    let pe_aip = build_x86_64_bezier_aip(15.0, 7.5);
    let plugin = Plugin::load(&pe_aip, Limits::default()).expect("load x86_64 PE32+ .aip");
    assert!(plugin.is_native_aip());
    assert_eq!(plugin.file_extension(), "aip");
    assert_eq!(plugin.manifest().api, 1);
    assert_eq!(plugin.manifest().id, "com.vectorcraft.translate_bezier");
    assert_eq!(plugin.manifest().name, "Translate Bezier C++");
    assert_eq!(plugin.manifest().kind, Kind::Filter);

    let (mut doc, id) = sample_curved_document();
    let input = objects::encode_input(&doc, &[id]);
    let raw_out = plugin.run_filter(&input, b"{}").expect("run x86_64 .aip filter");
    let decoded = objects::decode_output(&raw_out).expect("decode output");
    assert_eq!(decoded.objects.len(), 1);
    assert_eq!(decoded.objects[0].id, Some(id));

    let applied_ids = objects::apply_output(&mut doc, &[id], decoded.objects, None, &|fx, _| Ok(fx.to_vec())).expect("apply_output");
    assert_eq!(applied_ids, vec![id]);

    let updated = doc.node(id).expect("updated node");
    assert_eq!(updated.id, id);
    assert_eq!(updated.name.as_deref(), Some("Curved Badge"));
    assert!((updated.opacity - 0.75).abs() < 1e-6);
    assert_eq!(updated.appearance.items.len(), 2, "fill and stroke preserved");
    match &updated.appearance.items[1] {
        AppearanceItem::Stroke(s) => {
            assert!((s.width - 3.5).abs() < 1e-6);
            assert_eq!(s.join, LineJoin::Round);
            assert_eq!(s.cap, LineCap::Square);
        }
        other => panic!("expected stroke, got {other:?}"),
    }

    let NodeKind::Path { path, .. } = &updated.kind else { panic!("expected path") };
    assert!(path.subpaths[0].closed);
    let a0 = path.subpaths[0].anchors[0];
    assert!((a0.p.x - 35.0).abs() < 1e-6 && (a0.p.y - 37.5).abs() < 1e-6, "a0.p={:?}", a0.p);
    assert!((a0.h_in.x - 25.0).abs() < 1e-6 && (a0.h_in.y - 37.5).abs() < 1e-6, "a0.h_in={:?}", a0.h_in);
    assert!((a0.h_out.x - 50.0).abs() < 1e-6 && (a0.h_out.y - 37.5).abs() < 1e-6, "a0.h_out={:?}", a0.h_out);
    assert_eq!(a0.kind, AnchorKind::Smooth);

    let a2 = path.subpaths[0].anchors[2];
    assert!((a2.p.x - 35.0).abs() < 1e-6 && (a2.p.y - 127.5).abs() < 1e-6);
    assert_eq!(a2.kind, AnchorKind::Corner);
}

#[test]
fn clean_room_aarch64_macho_aip_runs_as_filter_live_effect_and_folder_plugin() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    vectorcraft_plugins::safe_mode::set_active(None);

    // 1. Live-effect Mach-O Universal `.bundle` registering via `AILiveEffectSuite`
    let arm64_effect_bin = build_arm64_bezier_aip(12.5, -4.25, true, true);
    let plugin = Plugin::load_with_hint(&arm64_effect_bin, Some("CurveOffset.bundle"), Limits::default()).expect("load arm64 .bundle");
    assert!(plugin.is_native_aip());
    assert_eq!(plugin.file_extension(), "bundle");
    assert_eq!(plugin.manifest().kind, Kind::Effect);
    assert_eq!(plugin.manifest().name, "Arm64 Curve Offset");
    assert_eq!(plugin.manifest().id, "aip.curveoffset");

    let effect_id = plugin.id().to_string();
    registry::install(plugin).expect("install arm64 live effect");

    let (doc, id) = sample_curved_document();
    let orig_path = doc.node(id).unwrap().path_data().unwrap().clone();
    let bounds = orig_path.bounds().unwrap_or(Rect::new(0.0, 0.0, 100.0, 100.0));
    let transformed =
        effect::apply(&effect_id, &json!({}), &orig_path, bounds, FillRule::NonZero, effect::Paints::default(), Some(&doc)).expect("apply effect");

    let b0 = transformed.subpaths[0].anchors[0];
    assert!((b0.p.x - 32.5).abs() < 1e-6 && (b0.p.y - 25.75).abs() < 1e-6, "b0.p={:?}", b0.p);
    assert!((b0.h_in.x - 22.5).abs() < 1e-6 && (b0.h_in.y - 25.75).abs() < 1e-6);
    assert!((b0.h_out.x - 47.5).abs() < 1e-6 && (b0.h_out.y - 25.75).abs() < 1e-6);
    assert!(registry::remove(&effect_id));

    // 2. Folder discovery of `.aip` and `.bundle` alongside `.wasm`
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = std::env::temp_dir().join(format!("vc-aip-folder-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let arm64_filter_bin = build_arm64_bezier_aip(5.0, 2.0, false, false);
        std::fs::write(dir.join("arm64_shift.aip"), &arm64_filter_bin).unwrap();
        let report = registry::load_folder(&dir).expect("load_folder with .aip");
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(report.loaded, vec!["aip.arm64_shift".to_string()], "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(registry::remove("aip.arm64_shift"));
    }
}

#[test]
fn loads_real_shipped_adobe_illustrator_2027_aip_plugins_if_present() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    vectorcraft_plugins::safe_mode::set_active(None);

    let candidates = [
        r"C:\Program Files\Adobe\Adobe Illustrator 2027\Plug-ins\Illustrator Filters\AddAnchor\AddAnchor.aip",
        r"C:\Program Files\Adobe\Adobe Illustrator 2027\Plug-ins\Illustrator Filters\AddAnchor.aip",
        r"C:\Program Files\Adobe\Adobe Illustrator 2027\Plug-ins\Illustrator Filters\Round\Round.aip",
        r"C:\Program Files\Adobe\Adobe Illustrator 2027\Plug-ins\Illustrator Filters\Round.aip",
    ];
    for raw_path in candidates {
        let path = Path::new(raw_path);
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(path).expect("read shipped .aip");
        let hint = path.file_name().and_then(|n| n.to_str());
        let plugin = Plugin::load_with_hint(&bytes, hint, Limits::default()).expect("Plugin::load_with_hint on real Illustrator 2027 .aip");
        assert!(plugin.is_native_aip());
        assert_eq!(plugin.manifest().api, 1);
        assert!(vectorcraft_plugins::manifest::valid_id(plugin.id()), "invalid id: {}", plugin.id());
        assert!(!plugin.manifest().name.is_empty());
        let desc = plugin.aip_descriptor().expect("aip_descriptor");
        assert_eq!(desc.entrypoint_symbol, "PluginMain");
    }
}
