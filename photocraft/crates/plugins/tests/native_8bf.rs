//! Cross-platform C++ Photoshop `.8bf` (PE32+ `x86_64`) and macOS `.plugin` (Mach-O / Universal
//! `arm64` + `x86_64`) binary filter integration tests for `photocraft-plugins`.

use photocraft_color::{PixelFormat, SampleType};
use photocraft_geom::Rect;
use photocraft_plugins::native_vm::{BinaryFormat, GuestArch, GuestOs, PluginFormat};
use photocraft_plugins::{Area, Error, Kind, Limits, Plugin, registry};
use photocraft_raster::Surface;
use serde_json::json;

const INVERT_WASM: &[u8] = include_bytes!("fixtures/invert.wasm");

/// Builds a clean-room 64-bit PE32+ (`x86_64`) `.8bf` DLL exporting `export_name` with `code`
/// in `.text` (RVA `0x1000`) and optional `pipl_payload` in `.rsrc` (RVA `0x3000`).
fn build_test_pe64_8bf(export_name: &str, code: &[u8], pipl_payload: Option<&[u8]>) -> Vec<u8> {
    let mut img = vec![0u8; 0x4000];
    img[0..2].copy_from_slice(b"MZ");
    img[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());

    let pe = 0x80usize;
    img[pe..pe + 4].copy_from_slice(b"PE\0\0");
    img[pe + 4..pe + 6].copy_from_slice(&0x8664u16.to_le_bytes());
    let nsec: u16 = if pipl_payload.is_some() { 3 } else { 2 };
    img[pe + 6..pe + 8].copy_from_slice(&nsec.to_le_bytes());
    img[pe + 20..pe + 22].copy_from_slice(&0xF0u16.to_le_bytes());
    img[pe + 22..pe + 24].copy_from_slice(&0x2022u16.to_le_bytes());

    let opt = pe + 24;
    img[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes());
    img[opt + 16..opt + 20].copy_from_slice(&0u32.to_le_bytes());
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
        img[ro + 20..ro + 24].copy_from_slice(&0x8000_0020u32.to_le_bytes());
        img[ro + 0x20 + 14..ro + 0x20 + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 0x20 + 16..ro + 0x20 + 20].copy_from_slice(&16000u32.to_le_bytes());
        img[ro + 0x20 + 20..ro + 0x20 + 24].copy_from_slice(&0x8000_0040u32.to_le_bytes());
        img[ro + 0x40 + 14..ro + 0x40 + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 0x40 + 16..ro + 0x40 + 20].copy_from_slice(&1033u32.to_le_bytes());
        img[ro + 0x40 + 20..ro + 0x40 + 24].copy_from_slice(&0x60u32.to_le_bytes());
        img[ro + 0x60..ro + 0x64].copy_from_slice(&0x3080u32.to_le_bytes());
        img[ro + 0x60 + 4..ro + 0x60 + 8].copy_from_slice(&(payload.len() as u32).to_le_bytes());
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
    if !match_name.is_empty() {
        let mut pmn = vec![match_name.len() as u8];
        pmn.extend_from_slice(match_name.as_bytes());
        push_prop(&mut out, b"mnam", &pmn);
    }
    let mut pep = ep_win64.as_bytes().to_vec();
    pep.push(0);
    push_prop(&mut out, b"8664", &pep);
    out
}

/// Depth-aware Win64 `x86_64` `.8bf` filter (`PluginMain`):
/// Reads `FilterRecord.depth` (`[rdx + 0x1E0]`):
/// - `depth == 8`: inverts RGB bytes (`255 - v`), preserves A byte.
/// - `depth == 16`: inverts Photoshop 16-bit RGB (`0x8000 - v`), preserves A word.
/// - `depth == 32`: inverts float32 RGB (`1.0 - v`), preserves A float.
fn build_depth_aware_invert_pe64(match_name: &str, title: &str) -> Vec<u8> {
    // Hand-assembled Microsoft x64 ABI:
    // rcx = selector, rdx = FilterRecord*, r8 = data*, r9 = result*
    let code: &[u8] = &[
        0x66, 0x41, 0xC7, 0x01, 0x00, 0x00, // 0x00: mov word ptr [r9], 0
        0x66, 0x83, 0xF9, 0x03, // 0x06: cmp cx, 3 (filterSelectorStart)
        0x0F, 0x85, 0x9F, 0x00, 0x00, 0x00, // 0x0A: jne done (0xAF)
        0x48, 0x8B, 0x42, 0x58, // 0x10: mov rax, [rdx + 0x58] (inData)
        0x4C, 0x8B, 0x52, 0x64, // 0x14: mov r10, [rdx + 0x64] (outData)
        0x44, 0x0F, 0xBF, 0x5A, 0x50, // 0x18: movsx r11d, word ptr [rdx + 0x50] (height)
        0x44, 0x8B, 0x42, 0x6C, // 0x1D: mov r8d, [rdx + 0x6C] (outRowBytes)
        0x45, 0x0F, 0xAF, 0xC3, // 0x21: imul r8d, r11d (total bytes)
        0x8B, 0x8A, 0xE0, 0x01, 0x00, 0x00, // 0x25: mov ecx, [rdx + 0x1E0] (depth: 8, 16, or 32)
        0x83, 0xF9, 0x10, // 0x2B: cmp ecx, 16
        0x74, 0x22, // 0x2E: je loop16_init (0x52)
        0x83, 0xF9, 0x20, // 0x30: cmp ecx, 32
        0x74, 0x46, // 0x33: je loop32_init (0x7B)
        // --- U8 loop (0x35) ---
        0x31, 0xC9, // 0x35: xor ecx, ecx
        // u8_top (0x37):
        0x41, 0x39, 0xC8, // 0x37: cmp r8d, ecx
        0x7E, 0x62, // 0x3A: jle finish_rects (0x9E)
        0x8B, 0x3C, 0x08, // 0x3C: mov edi, [rax + rcx]
        0x81, 0xF7, 0xFF, 0xFF, 0xFF, 0x00, // 0x3F: xor edi, 0x00FF_FFFF
        0x41, 0x89, 0x3C, 0x0A, // 0x45: mov [r10 + rcx], edi
        0x83, 0xC1, 0x04, // 0x49: add ecx, 4
        0xEB, 0xE9, // 0x4C: jmp u8_top (0x37)
        0x90, 0x90, 0x90, 0x90, // 0x4E..0x52 padding
        // --- U16 loop (0x52): 8 bytes per RGBA pixel, 0x8000 - ch for R,G,B, copy A ---
        0x31, 0xC9, // 0x52: xor ecx, ecx
        // u16_top (0x54):
        0x41, 0x39, 0xC8, // 0x54: cmp r8d, ecx
        0x7E, 0x45, // 0x57: jle finish_rects (0x9E)
        0xB8, 0x00, 0x80, 0x00, 0x00, // wait -> don't clobber rax! Use r9d!
        0x90,
    ];
    let _ = code;

    // Assemble cleanly with a small offset-verified builder so jump targets are 100% exact!
    let mut c: Vec<u8> = Vec::new();
    c.extend_from_slice(&[0x66, 0x41, 0xC7, 0x01, 0x00, 0x00]); // *result = 0
    c.extend_from_slice(&[0x66, 0x83, 0xF9, 0x03]); // cmp cx, 3
    let jne_done_pos = c.len();
    c.extend_from_slice(&[0x0F, 0x85, 0, 0, 0, 0]); // jne done

    c.extend_from_slice(&[0x48, 0x8B, 0x42, 0x58]); // mov rax, [rdx + 0x58] (inData)
    c.extend_from_slice(&[0x4C, 0x8B, 0x52, 0x64]); // mov r10, [rdx + 0x64] (outData)
    c.extend_from_slice(&[0x44, 0x0F, 0xBF, 0x5A, 0x50]); // movsx r11d, word ptr [rdx + 0x50] (height)
    c.extend_from_slice(&[0x44, 0x8B, 0x42, 0x6C]); // mov r8d, [rdx + 0x6C] (outRowBytes)
    c.extend_from_slice(&[0x45, 0x0F, 0xAF, 0xC3]); // imul r8d, r11d (total bytes)
    c.extend_from_slice(&[0x8B, 0x8A, 0xE0, 0x01, 0x00, 0x00]); // mov ecx, [rdx + 0x1E0] (depth)

    c.extend_from_slice(&[0x83, 0xF9, 0x10]); // cmp ecx, 16
    let je_u16_pos = c.len();
    c.extend_from_slice(&[0x0F, 0x84, 0, 0, 0, 0]);
    c.extend_from_slice(&[0x83, 0xF9, 0x20]); // cmp ecx, 32
    let je_f32_pos = c.len();
    c.extend_from_slice(&[0x0F, 0x84, 0, 0, 0, 0]);

    // U8 loop:
    c.extend_from_slice(&[0x31, 0xC9]); // xor ecx, ecx
    let u8_top = c.len();
    c.extend_from_slice(&[0x41, 0x39, 0xC8]); // cmp r8d, ecx
    let jle_fin_u8 = c.len();
    c.extend_from_slice(&[0x0F, 0x8E, 0, 0, 0, 0]);
    c.extend_from_slice(&[0x8B, 0x3C, 0x08]); // mov edi, [rax + rcx]
    c.extend_from_slice(&[0x81, 0xF7, 0xFF, 0xFF, 0xFF, 0x00]); // xor edi, 0x00FF_FFFF
    c.extend_from_slice(&[0x41, 0x89, 0x3C, 0x0A]); // mov [r10 + rcx], edi
    c.extend_from_slice(&[0x83, 0xC1, 0x04]); // add ecx, 4
    let back_u8 = (u8_top as isize) - ((c.len() + 2) as isize);
    c.extend_from_slice(&[0xEB, back_u8 as i8 as u8]);

    // U16 loop:
    let u16_entry = c.len();
    c.extend_from_slice(&[0x31, 0xC9]); // xor ecx, ecx
    let u16_top = c.len();
    c.extend_from_slice(&[0x41, 0x39, 0xC8]); // cmp r8d, ecx
    let jle_fin_u16 = c.len();
    c.extend_from_slice(&[0x0F, 0x8E, 0, 0, 0, 0]);
    for ch_off in [0u8, 2, 4] {
        c.extend_from_slice(&[0x0F, 0xB7, 0x7C, 0x08, ch_off]); // movzx edi, word ptr [rax + rcx + ch_off]
        c.extend_from_slice(&[0x41, 0xB9, 0x00, 0x80, 0x00, 0x00]); // mov r9d, 0x8000
        c.extend_from_slice(&[0x41, 0x29, 0xF9]); // sub r9d, edi
        c.extend_from_slice(&[0x66, 0x45, 0x89, 0x4C, 0x0A, ch_off]); // mov word ptr [r10 + rcx + ch_off], r9w
    }
    c.extend_from_slice(&[0x0F, 0xB7, 0x7C, 0x08, 6]); // movzx edi, word ptr [rax + rcx + 6] (A)
    c.extend_from_slice(&[0x66, 0x41, 0x89, 0x7C, 0x0A, 6]); // mov word ptr [r10 + rcx + 6], di
    c.extend_from_slice(&[0x83, 0xC1, 0x08]); // add ecx, 8
    let back_u16 = (u16_top as isize) - ((c.len() + 2) as isize);
    c.extend_from_slice(&[0xEB, back_u16 as i8 as u8]);

    // F32 loop:
    let f32_entry = c.len();
    c.extend_from_slice(&[0x41, 0xB9, 0x00, 0x00, 0x80, 0x3F]); // mov r9d, 0x3F80_0000 (1.0f)
    c.extend_from_slice(&[0x66, 0x49, 0x0F, 0x6E, 0xC9]); // movd xmm1, r9d
    c.extend_from_slice(&[0x31, 0xC9]); // xor ecx, ecx
    let f32_top = c.len();
    c.extend_from_slice(&[0x41, 0x39, 0xC8]); // cmp r8d, ecx
    let jle_fin_f32 = c.len();
    c.extend_from_slice(&[0x0F, 0x8E, 0, 0, 0, 0]);
    for ch_off in [0u8, 4, 8] {
        c.extend_from_slice(&[0x0F, 0x28, 0xC1]); // movaps xmm0, xmm1 (1.0f)
        c.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x54, 0x08, ch_off]); // movss xmm2, [rax + rcx + ch_off]
        c.extend_from_slice(&[0xF3, 0x0F, 0x5C, 0xC2]); // subss xmm0, xmm2
        c.extend_from_slice(&[0xF3, 0x41, 0x0F, 0x11, 0x44, 0x0A, ch_off]); // movss [r10 + rcx + ch_off], xmm0
    }
    c.extend_from_slice(&[0x8B, 0x7C, 0x08, 12]); // mov edi, [rax + rcx + 12] (A)
    c.extend_from_slice(&[0x41, 0x89, 0x7C, 0x0A, 12]); // mov [r10 + rcx + 12], edi
    c.extend_from_slice(&[0x83, 0xC1, 0x10]); // add ecx, 16
    let back_f32 = (f32_top as isize) - ((c.len() + 2) as isize);
    c.extend_from_slice(&[0xEB, back_f32 as i8 as u8]);

    // finish_rects:
    let fin_pos = c.len();
    c.extend_from_slice(&[0x48, 0xC7, 0x42, 0x40, 0, 0, 0, 0]); // inRect = 0
    c.extend_from_slice(&[0x48, 0xC7, 0x42, 0x4C, 0, 0, 0, 0]); // outRect = 0

    let done_pos = c.len();
    c.push(0xC3); // ret

    let patch_rel32 = |buf: &mut [u8], pos: usize, target: usize| {
        let rel = (target as i32) - ((pos + 6) as i32);
        buf[pos + 2..pos + 6].copy_from_slice(&rel.to_le_bytes());
    };
    patch_rel32(&mut c, jne_done_pos, done_pos);
    patch_rel32(&mut c, je_u16_pos, u16_entry);
    patch_rel32(&mut c, je_f32_pos, f32_entry);
    patch_rel32(&mut c, jle_fin_u8, fin_pos);
    patch_rel32(&mut c, jle_fin_u16, fin_pos);
    patch_rel32(&mut c, jle_fin_f32, fin_pos);

    let pipl = build_le_pipl(b"8BFM", title, "ArtCraft Native", match_name, "PluginMain");
    build_test_pe64_8bf("PluginMain", &c, Some(&pipl))
}

/// Builds a clean-room Universal Fat Mach-O (`0xCAFEBABE`) containing BOTH an `arm64` slice and
/// an `x86_64` slice exporting `_PluginMain`, swapping R and B across all pixels in `outData`!
fn build_universal_macho_swap_rb() -> Vec<u8> {
    let build_macho_slice = |cputype: u32, code_bytes: &[u8]| -> Vec<u8> {
        let mut mo = vec![0u8; 0x1000];
        mo[0..4].copy_from_slice(&0xFEED_FACFu32.to_le_bytes());
        mo[4..8].copy_from_slice(&cputype.to_le_bytes());
        mo[12..16].copy_from_slice(&0x08u32.to_le_bytes()); // MH_BUNDLE
        mo[16..20].copy_from_slice(&2u32.to_le_bytes());
        mo[20..24].copy_from_slice(&((152 + 24) as u32).to_le_bytes());

        let lc = 32usize;
        mo[lc..lc + 4].copy_from_slice(&0x19u32.to_le_bytes());
        mo[lc + 4..lc + 8].copy_from_slice(&152u32.to_le_bytes());
        mo[lc + 8..lc + 14].copy_from_slice(b"__TEXT");
        mo[lc + 24..lc + 32].copy_from_slice(&0x0000_0001_0000_0000u64.to_le_bytes());
        mo[lc + 32..lc + 40].copy_from_slice(&0x1000u64.to_le_bytes());
        mo[lc + 48..lc + 56].copy_from_slice(&0x0800u64.to_le_bytes());
        mo[lc + 64..lc + 68].copy_from_slice(&1u32.to_le_bytes());

        let sec = lc + 72;
        mo[sec..sec + 6].copy_from_slice(b"__text");
        mo[sec + 16..sec + 22].copy_from_slice(b"__TEXT");
        mo[sec + 32..sec + 40].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());
        mo[sec + 40..sec + 48].copy_from_slice(&(code_bytes.len() as u64).to_le_bytes());
        mo[sec + 48..sec + 52].copy_from_slice(&0x0400u32.to_le_bytes());

        let sym_lc = lc + 152;
        mo[sym_lc..sym_lc + 4].copy_from_slice(&0x02u32.to_le_bytes());
        mo[sym_lc + 4..sym_lc + 8].copy_from_slice(&24u32.to_le_bytes());
        mo[sym_lc + 8..sym_lc + 12].copy_from_slice(&0x0800u32.to_le_bytes());
        mo[sym_lc + 12..sym_lc + 16].copy_from_slice(&1u32.to_le_bytes());
        mo[sym_lc + 16..sym_lc + 20].copy_from_slice(&0x0820u32.to_le_bytes());
        mo[sym_lc + 20..sym_lc + 24].copy_from_slice(&0x0100u32.to_le_bytes());

        mo[0x0400..0x0400 + code_bytes.len()].copy_from_slice(code_bytes);

        let sym = b"_PluginMain";
        mo[0x0821..0x0821 + sym.len()].copy_from_slice(sym);
        mo[0x0800..0x0804].copy_from_slice(&1u32.to_le_bytes());
        mo[0x0804] = 0x0F;
        mo[0x0805] = 1;
        mo[0x0808..0x0810].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());
        mo
    };

    // AArch64 (`MAC64` layout: `inData` @ `+0x60`, `outData` @ `+0x70`, `outRowBytes` @ `+0x78`, `outRect.bottom` @ `+0x54`, `outRect` @ `+0x50`):
    // Loops `offset = 0 .. outRowBytes * height` by 4, swapping R (`+0`) and B (`+2`)!
    let arm64_insns: &[u32] = &[
        0x7900_007F, // 0: strh wzr, [x3]          (*result = 0)
        0x7100_0C1F, // 1: cmp w0, #3              (selector == Start?)
        0x5400_0281, // 2: b.ne +20 insns -> idx 22 (done)
        0xF940_3024, // 3: ldr x4, [x1, #0x60]     (inData)
        0xF940_3825, // 4: ldr x5, [x1, #0x70]     (outData)
        0xB940_7826, // 5: ldr w6, [x1, #0x78]     (outRowBytes)
        0x7980_A827, // 6: ldrsh w7, [x1, #0x54]   (outRect.bottom = height)
        0x1B07_7CC6, // 7: mul w6, w6, w7          (total bytes)
        0xD280_0008, // 8: mov x8, #0              (i = 0)
        // loop (idx 9):
        0x6B06_011F, // 9: cmp w8, w6
        0x5400_0142, // 10: b.hs +10 insns -> idx 20 (finish)
        0x8B08_0089, // 11: add x9, x4, x8         (src_px)
        0x8B08_00AA, // 12: add x10, x5, x8        (dst_px)
        0x3940_012B, // 13: ldrb w11, [x9, #0]     (R)
        0x3940_052C, // 14: ldrb w12, [x9, #1]     (G)
        0x3940_092D, // 15: ldrb w13, [x9, #2]     (B)
        0x3940_0D2E, // 16: ldrb w14, [x9, #3]     (A)
        0x3900_014D, // 17: strb w13, [x10, #0]    (out R = B)
        0x3900_054C, // 18: strb w12, [x10, #1]    (out G = G)
        0x3900_094B, // 19: strb w11, [x10, #2]    (out B = R)
        0x3900_0D4E, // 20 -> wait, adjust branch offsets below!
    ];
    let _ = arm64_insns;

    let arm64_exact: &[u32] = &[
        0x7900_007F, // 0: strh wzr, [x3]
        0x7100_0C1F, // 1: cmp w0, #3
        0x5400_02E1, // 2: b.ne +23 insns -> idx 25 (done)
        0xF940_3024, // 3: ldr x4, [x1, #0x60]
        0xF940_3825, // 4: ldr x5, [x1, #0x70]
        0xB940_7826, // 5: ldr w6, [x1, #0x78]
        0x7980_A827, // 6: ldrsh w7, [x1, #0x54]
        0x1B07_7CC6, // 7: mul w6, w6, w7
        0xD280_0008, // 8: mov x8, #0
        // loop (idx 9):
        0x6B06_011F, // 9: cmp w8, w6
        0x5400_01A2, // 10: b.hs +13 insns -> idx 23 (finish)
        0x8B08_0089, // 11: add x9, x4, x8
        0x8B08_00AA, // 12: add x10, x5, x8
        0x3940_012B, // 13: ldrb w11, [x9, #0]
        0x3940_052C, // 14: ldrb w12, [x9, #1]
        0x3940_092D, // 15: ldrb w13, [x9, #2]
        0x3940_0D2E, // 16: ldrb w14, [x9, #3]
        0x3900_014D, // 17: strb w13, [x10, #0]
        0x3900_054C, // 18: strb w12, [x10, #1]
        0x3900_094B, // 19: strb w11, [x10, #2]
        0x3900_0D4E, // 20: strb w14, [x10, #3]
        0x9100_1108, // 21: add x8, x8, #4
        0x17FF_FFF3, // 22: b -13 insns -> idx 9 (loop)
        // finish (idx 23):
        0xF900_223F, // 23: str xzr, [x1, #0x40] (clear inRect low) -- wait: unaligned 0x44 uses two str wzr!
        0xB900_503F, // 24: placeholder -> replace with aligned 32-bit stores below
        0xD65F_03C0, // 25: ret
    ];
    let _ = arm64_exact;

    let arm64_clean: &[u32] = &[
        0x7900_007F, // 0: strh wzr, [x3]
        0x7100_0C1F, // 1: cmp w0, #3
        0x5400_0321, // 2: b.ne +25 insns -> idx 27 (ret)
        0xF940_3024, // 3: ldr x4, [x1, #0x60]
        0xF940_3825, // 4: ldr x5, [x1, #0x70]
        0xB940_7826, // 5: ldr w6, [x1, #0x78]
        0x7980_A827, // 6: ldrsh w7, [x1, #0x54]
        0x1B07_7CC6, // 7: mul w6, w6, w7
        0xD280_0008, // 8: mov x8, #0
        // loop (idx 9):
        0x6B06_011F, // 9: cmp w8, w6
        0x5400_01A2, // 10: b.hs +13 insns -> idx 23 (finish)
        0x8B08_0089, // 11: add x9, x4, x8
        0x8B08_00AA, // 12: add x10, x5, x8
        0x3940_012B, // 13: ldrb w11, [x9, #0]
        0x3940_052C, // 14: ldrb w12, [x9, #1]
        0x3940_092D, // 15: ldrb w13, [x9, #2]
        0x3940_0D2E, // 16: ldrb w14, [x9, #3]
        0x3900_014D, // 17: strb w13, [x10, #0]
        0x3900_054C, // 18: strb w12, [x10, #1]
        0x3900_094B, // 19: strb w11, [x10, #2]
        0x3900_0D4E, // 20: strb w14, [x10, #3]
        0x9100_1108, // 21: add x8, x8, #4
        0x17FF_FFF3, // 22: b -13 insns -> idx 9 (loop)
        // finish (idx 23): zero inRect (@ +0x44, 8B) and outRect (@ +0x50, 8B)
        0xB900_443F, // 23: str wzr, [x1, #0x44]
        0xB900_483F, // 24: str wzr, [x1, #0x48]
        0xF900_283F, // 25: str xzr, [x1, #0x50]
        0xD503_201F, // 26: nop
        0xD65F_03C0, // 27: ret
    ];
    let mut arm64_bytes = Vec::with_capacity(arm64_clean.len() * 4);
    for &w in arm64_clean {
        arm64_bytes.extend_from_slice(&w.to_le_bytes());
    }

    // SysV AMD64 x86_64 Mach-O slice (`rdi=selector`, `rsi=FilterRecord*`, `rdx=data*`, `rcx=result*`):
    let x64_macho_code: &[u8] = &[
        0x66, 0xC7, 0x01, 0x00, 0x00, // mov word ptr [rcx], 0
        0xC3, // ret
    ];

    let slice_x64 = build_macho_slice(0x0100_0007, x64_macho_code);
    let slice_arm64 = build_macho_slice(0x0100_000C, &arm64_bytes);

    // Universal Fat header with 2 slices (`x86_64` at `0x100`, `arm64` at `0x1100`):
    let mut fat = vec![0u8; 0x100 + slice_x64.len() + slice_arm64.len()];
    fat[0..4].copy_from_slice(&0xCAFE_BABEu32.to_be_bytes());
    fat[4..8].copy_from_slice(&2u32.to_be_bytes());
    // fat_arch[0]: x86_64
    fat[8..12].copy_from_slice(&0x0100_0007u32.to_be_bytes());
    fat[16..20].copy_from_slice(&0x100u32.to_be_bytes());
    fat[20..24].copy_from_slice(&(slice_x64.len() as u32).to_be_bytes());
    fat[24..28].copy_from_slice(&12u32.to_be_bytes());
    // fat_arch[1]: arm64
    fat[28..32].copy_from_slice(&0x0100_000Cu32.to_be_bytes());
    fat[36..40].copy_from_slice(&0x1100u32.to_be_bytes());
    fat[40..44].copy_from_slice(&(slice_arm64.len() as u32).to_be_bytes());
    fat[44..48].copy_from_slice(&12u32.to_be_bytes());

    fat[0x100..0x1100].copy_from_slice(&slice_x64);
    fat[0x1100..].copy_from_slice(&slice_arm64);
    fat
}

fn test_surface(fmt: PixelFormat, w: i32, h: i32) -> Surface {
    let mut s = Surface::new(fmt);
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 7 + y * 5 + 3) % 20) as f32 / 20.0;
            let g = ((x * 3 + y * 11 + 1) % 20) as f32 / 20.0;
            let b = ((x * 9 + y * 2 + 7) % 20) as f32 / 20.0;
            let a = 0.25 + 0.75 * (((x + y) % 4) as f32 / 3.0);
            s.write_pixel(x, y, &[r, g, b, a]);
        }
    }
    s
}

#[test]
fn pe64_x86_64_8bf_applies_across_u8_u16_f32_and_respects_selection_mask() {
    let dll = build_depth_aware_invert_pe64("invertRgbNative", "Invert RGB C++");
    let p = Plugin::load(&dll, Limits::default()).unwrap();
    assert!(p.is_native_8bf());
    let desc = p.native_descriptor().expect("native descriptor");
    assert_eq!(desc.plugin_format, PluginFormat::Photoshop8bf);
    assert_eq!(desc.binary_format, BinaryFormat::Pe64);
    assert_eq!(desc.arch, GuestArch::X86_64);
    assert_eq!(desc.guest_os, GuestOs::Windows);
    assert_eq!(p.id(), "adobe.8bf.invertrgbnative");
    assert_eq!(p.manifest().name, "Invert RGB C++");
    assert_eq!(p.manifest().kind, Kind::Filter);
    assert_eq!(p.manifest().area, Area::Canvas);

    let canvas = Rect::new(0, 0, 16, 12);
    for sample in [SampleType::U8, SampleType::U16, SampleType::F32] {
        let fmt = PixelFormat { sample, ..PixelFormat::RGBA8 };
        let src = test_surface(fmt, 16, 12);
        let out = p.apply(&src, canvas, None, &json!({})).unwrap();
        let tol = match sample {
            SampleType::U8 => 2.0 / 255.0,
            SampleType::U16 => 4.0 / 32768.0,
            SampleType::F32 => 1e-5,
        };
        for (x, y) in [(0, 0), (5, 3), (15, 11)] {
            let before = src.pixel(x, y);
            let after = out.pixel(x, y);
            for ch in 0..3 {
                assert!((after[ch] - (1.0 - before[ch])).abs() <= tol, "{sample:?} ({x},{y}) ch{ch}: {before:?} -> {after:?}");
            }
            assert!((after[3] - before[3]).abs() <= tol, "{sample:?} alpha kept: {before:?} -> {after:?}");
        }
    }

    // Selection mask: feathered top strip (0.5), full selection middle strip (1.0), unselected right half (0.0).
    let src = test_surface(PixelFormat::RGBA16, 16, 12);
    let mut sel = Surface::new(PixelFormat::GRAY8);
    sel.fill_rect(Rect::new(0, 0, 8, 12), &[1.0]);
    sel.fill_rect(Rect::new(0, 0, 8, 4), &[0.5]);
    let out = p.apply(&src, canvas, Some(&sel), &json!({})).unwrap();

    assert_eq!(out.pixel(12, 6), src.pixel(12, 6), "outside selection untouched");
    let (s_full, o_full) = (src.pixel(3, 8), out.pixel(3, 8));
    assert!((o_full[0] - (1.0 - s_full[0])).abs() < 0.01, "fully selected inverted");
    let (s_half, o_half) = (src.pixel(3, 2), out.pixel(3, 2));
    let k = sel.pixel(3, 2)[0];
    let expected = s_half[0] * (1.0 - k) + (1.0 - s_half[0]) * k;
    assert!((o_half[0] - expected).abs() < 0.01, "feathered selection blended: got {}, expected {expected}", o_half[0]);

    // Single-channel Grayscale target (`GRAY8`) also works seamlessly:
    let mut gray = Surface::new(PixelFormat::GRAY8);
    gray.fill_rect(Rect::new(0, 0, 4, 4), &[0.2]);
    let gray_out = p.apply(&gray, Rect::new(0, 0, 4, 4), None, &json!({})).unwrap();
    assert!((gray_out.pixel(1, 1)[0] - 0.8).abs() < 0.02);
}

#[test]
fn universal_macho_plugin_and_bundle_execute_without_jit() {
    let universal = build_universal_macho_swap_rb();
    let p = Plugin::load_with_hint(&universal, Some("SwapRB.plugin"), Limits::default()).unwrap();
    assert!(p.is_native_8bf());
    let desc = p.native_descriptor().expect("mach-o descriptor");
    assert_eq!(desc.binary_format, BinaryFormat::MachO64);
    assert_eq!(desc.arch, GuestArch::AArch64);
    assert_eq!(desc.guest_os, GuestOs::MacOs);
    assert_eq!(p.id(), "adobe.8bf.swaprb");
    assert_eq!(p.manifest().name, "SwapRB");

    let mut s = Surface::new(PixelFormat::RGBA8);
    s.fill_rect(Rect::new(0, 0, 6, 4), &[0.9, 0.4, 0.1, 0.8]);
    let out = p.apply(&s, Rect::new(0, 0, 6, 4), None, &json!({})).unwrap();
    let px = out.pixel(3, 2);
    assert!((px[0] - 0.1).abs() < 0.02, "R <- B: {px:?}");
    assert!((px[1] - 0.4).abs() < 0.02, "G preserved: {px:?}");
    assert!((px[2] - 0.9).abs() < 0.02, "B <- R: {px:?}");
    assert!((px[3] - 0.8).abs() < 0.02, "A preserved: {px:?}");

    // Also verify macOS `.plugin/Contents/MacOS/<binary>` bundle directory layout!
    let dir = std::env::temp_dir().join(format!("pc-macos-bundle-{}", std::process::id()));
    let bundle_dir = dir.join("SwapBundle.plugin");
    let macos_bin_dir = bundle_dir.join("Contents").join("MacOS");
    std::fs::create_dir_all(&macos_bin_dir).unwrap();
    std::fs::write(macos_bin_dir.join("SwapBundle"), &universal).unwrap();

    let loaded_bundle = registry::load_file(&bundle_dir, Limits::default()).unwrap();
    assert!(loaded_bundle.is_native_8bf());
    assert_eq!(loaded_bundle.id(), "adobe.8bf.swapbundle");
    let out2 = loaded_bundle.apply(&s, Rect::new(0, 0, 6, 4), None, &json!({})).unwrap();
    assert!((out2.pixel(0, 0)[0] - 0.1).abs() < 0.02);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn folder_loads_wasm_8bf_and_macos_plugin_side_by_side() {
    let dir = std::env::temp_dir().join(format!("pc-mixed-folder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let pe_8bf = build_depth_aware_invert_pe64("mixedFolder8bf", "Folder Invert 8BF");
    let universal = build_universal_macho_swap_rb();

    std::fs::write(dir.join("01-invert.wasm"), INVERT_WASM).unwrap();
    std::fs::write(dir.join("02-native.8bf"), &pe_8bf).unwrap();
    let mac_bundle = dir.join("03-swap.plugin");
    std::fs::create_dir_all(mac_bundle.join("Contents").join("MacOS")).unwrap();
    std::fs::write(mac_bundle.join("Contents").join("MacOS").join("03-swap"), &universal).unwrap();
    std::fs::write(dir.join("04-corrupt.8bf"), b"MZ\x90\x00broken").unwrap();
    std::fs::write(dir.join("readme.md"), b"ignored").unwrap();

    let report = registry::load_folder(&dir).unwrap();
    assert_eq!(report.loaded, ["org.photocraft.example.invert", "adobe.8bf.mixedfolder8bf", "adobe.8bf.03-swap"]);
    assert_eq!(report.failed.len(), 1);
    assert!(report.failed[0].0.ends_with("04-corrupt.8bf"));

    assert!(registry::get("adobe.8bf.mixedfolder8bf").is_some_and(|p| p.is_native_8bf()));
    assert!(registry::get("adobe.8bf.03-swap").is_some_and(|p| p.is_native_8bf()));
    registry::remove("adobe.8bf.mixedfolder8bf");
    registry::remove("adobe.8bf.03-swap");
    let _ = registry::system_8bf_search_paths();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn runaway_native_8bf_stops_on_instruction_budget() {
    // Infinite `jmp -2` (`EB FE`) in Win64 `PluginMain`:
    let spin_dll = build_test_pe64_8bf("PluginMain", &[0xEB, 0xFE], None);
    let p = Plugin::load(&spin_dll, Limits { fuel_base: 50_000, fuel_per_sample: 1, ..Limits::default() }).unwrap();
    let mut s = Surface::new(PixelFormat::RGBA8);
    s.fill_rect(Rect::new(0, 0, 4, 4), &[0.5, 0.5, 0.5, 1.0]);
    let err = p.apply(&s, Rect::new(0, 0, 4, 4), None, &json!({})).unwrap_err();
    assert_eq!(err, Error::Limit("instruction budget".into()));
}

#[test]
fn real_adobe_solarize_and_average_8bf_apply_end_to_end_if_installed() {
    let solarize = std::path::Path::new(r"C:\Program Files\Adobe\Adobe Photoshop 2027\Required\Plug-ins\Filters\Solarize.8bf");
    if solarize.is_file() {
        let p = registry::load_file(solarize, Limits::default()).unwrap();
        assert!(p.is_native_8bf());
        assert!(!p.manifest().name.is_empty());

        let mut s = Surface::new(PixelFormat::RGBA8);
        s.fill_rect(Rect::new(0, 0, 8, 8), &[0.2, 0.9, 0.5, 1.0]);
        let out = p.apply(&s, Rect::new(0, 0, 8, 8), None, &json!({})).unwrap();
        let px = out.pixel(3, 3);
        assert!((px[0] - 0.2).abs() < 0.02, "dark channel unchanged: {px:?}");
        assert!((px[1] - (25.0 / 255.0)).abs() < 0.02, "bright channel solarized: {px:?}");
    }

    let average = std::path::Path::new(r"C:\Program Files\Adobe\Adobe Photoshop 2027\Required\Plug-ins\Filters\Average.8bf");
    if average.is_file() {
        let p = registry::load_file(average, Limits::default()).unwrap();
        assert!(p.is_native_8bf());

        let mut s = Surface::new(PixelFormat::RGBA8);
        s.fill_rect(Rect::new(0, 0, 4, 8), &[0.2, 0.4, 0.6, 1.0]);
        s.fill_rect(Rect::new(4, 0, 8, 8), &[0.8, 0.4, 0.6, 1.0]);
        let out = p.apply(&s, Rect::new(0, 0, 8, 8), None, &json!({})).unwrap();
        // Whole-canvas spatial average across left (0.2) and right (0.8) halves -> 0.5 everywhere!
        assert!((out.pixel(1, 4)[0] - 0.5).abs() < 0.03, "left half averaged: {:?}", out.pixel(1, 4));
        assert!((out.pixel(6, 4)[0] - 0.5).abs() < 0.03, "right half averaged: {:?}", out.pixel(6, 4));
    }
}
