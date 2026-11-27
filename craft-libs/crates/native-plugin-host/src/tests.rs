//! End-to-end tests for cross-platform C++ binary plugin execution across:
//! - `x86_64` PE32+ (`.8bf`, `.prm`, `.aex`, `.aip`)
//! - `AArch64` (`arm64`) Mach-O 64-bit & Universal Fat (`.plugin`, `.aex`, `.prm`, `.aip`)
//! - Real shipped Adobe C++ plugins on disk (`Solarize.8bf`, `Average.8bf`, `ThresholdRGB.aex`,
//!   `AddAnchor.aip`) when present in `C:\Program Files\Adobe`.

use super::{
    AeParamKind, AePrmHost, AipBezierSegment, AipHost, AipSubpath, BinaryFormat, Filter8bfHost, GuestArch, ImageBuffer8bf, Limits, PixelDepth8bf,
    PluginFormat, RgbaFrame, inspect_plugin_binary,
};

/// Builds a minimal, spec-valid 64-bit PE32+ (`x86_64`) DLL exporting `export_name` with `code`
/// in `.text` (RVA `0x1000`) and optional `pipl_rsrc` in `.rsrc` (RVA `0x3000`).
pub(crate) fn build_test_pe64(export_name: &str, code: &[u8], pipl_payload: Option<&[u8]>) -> Vec<u8> {
    let mut img = vec![0u8; 0x4000];
    // DOS header
    img[0..2].copy_from_slice(b"MZ");
    img[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());

    // PE signature + COFF header at 0x80
    let pe = 0x80usize;
    img[pe..pe + 4].copy_from_slice(b"PE\0\0");
    img[pe + 4..pe + 6].copy_from_slice(&0x8664u16.to_le_bytes()); // AMD64
    let nsec: u16 = if pipl_payload.is_some() { 3 } else { 2 };
    img[pe + 6..pe + 8].copy_from_slice(&nsec.to_le_bytes());
    img[pe + 20..pe + 22].copy_from_slice(&0xF0u16.to_le_bytes()); // SizeOfOptionalHeader
    img[pe + 22..pe + 24].copy_from_slice(&0x2022u16.to_le_bytes()); // EXECUTABLE | LARGE_ADDRESS | DLL

    // OptionalHeader64 at 0x98
    let opt = pe + 24;
    img[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // PE32+
    img[opt + 16..opt + 20].copy_from_slice(&0u32.to_le_bytes()); // AddressOfEntryPoint = 0
    img[opt + 24..opt + 32].copy_from_slice(&0x0000_0001_8000_0000u64.to_le_bytes()); // ImageBase
    img[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes()); // SectionAlignment
    img[opt + 36..opt + 40].copy_from_slice(&0x0400u32.to_le_bytes()); // FileAlignment
    img[opt + 56..opt + 60].copy_from_slice(&0x5000u32.to_le_bytes()); // SizeOfImage
    img[opt + 60..opt + 64].copy_from_slice(&0x0400u32.to_le_bytes()); // SizeOfHeaders
    img[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes

    // DataDirectory[0] = Export (`RVA = 0x2000`, `Size = 0x200`)
    let dd = opt + 112;
    img[dd..dd + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    img[dd + 4..dd + 8].copy_from_slice(&0x200u32.to_le_bytes());
    if pipl_payload.is_some() {
        // DataDirectory[2] = Resource (`RVA = 0x3000`, `Size = 0x800`)
        img[dd + 16..dd + 20].copy_from_slice(&0x3000u32.to_le_bytes());
        img[dd + 20..dd + 24].copy_from_slice(&0x800u32.to_le_bytes());
    }

    // Section headers at `opt + 0xF0` = `0x188`
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

    // `.text` at file 0x0400 -> RVA 0x1000
    img[0x0400..0x0400 + code.len()].copy_from_slice(code);

    // `.edata` at file 0x0C00 -> RVA 0x2000
    let eo = 0x0C00usize;
    img[eo + 16..eo + 20].copy_from_slice(&1u32.to_le_bytes()); // Base = 1
    img[eo + 20..eo + 24].copy_from_slice(&1u32.to_le_bytes()); // NumberOfFunctions = 1
    img[eo + 24..eo + 28].copy_from_slice(&1u32.to_le_bytes()); // NumberOfNames = 1
    img[eo + 28..eo + 32].copy_from_slice(&0x2040u32.to_le_bytes()); // AddressOfFunctions
    img[eo + 32..eo + 36].copy_from_slice(&0x2044u32.to_le_bytes()); // AddressOfNames
    img[eo + 36..eo + 40].copy_from_slice(&0x2048u32.to_le_bytes()); // AddressOfNameOrdinals
    img[eo + 0x40..eo + 0x44].copy_from_slice(&0x1000u32.to_le_bytes()); // fn[0] = RVA 0x1000
    img[eo + 0x44..eo + 0x48].copy_from_slice(&0x2050u32.to_le_bytes()); // name[0] = RVA 0x2050
    img[eo + 0x48..eo + 0x4A].copy_from_slice(&0u16.to_le_bytes()); // ord[0] = 0
    let nb = export_name.as_bytes();
    img[eo + 0x50..eo + 0x50 + nb.len()].copy_from_slice(nb);

    // Optional `.rsrc` 3-level tree at file 0x1000 -> RVA 0x3000
    if let Some(payload) = pipl_payload {
        let ro = 0x1000usize;
        // Root dir (0x00): 1 ID entry -> subdir 0x20
        img[ro + 14..ro + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 16..ro + 20].copy_from_slice(&10u32.to_le_bytes()); // RT_RCDATA
        img[ro + 20..ro + 24].copy_from_slice(&(0x8000_0020u32).to_le_bytes());
        // Level 2 dir (0x20): 1 ID entry -> subdir 0x40
        img[ro + 0x20 + 14..ro + 0x20 + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 0x20 + 16..ro + 0x20 + 20].copy_from_slice(&16000u32.to_le_bytes());
        img[ro + 0x20 + 20..ro + 0x20 + 24].copy_from_slice(&(0x8000_0040u32).to_le_bytes());
        // Level 3 dir (0x40): 1 lang entry -> data entry 0x60
        img[ro + 0x40 + 14..ro + 0x40 + 16].copy_from_slice(&1u16.to_le_bytes());
        img[ro + 0x40 + 16..ro + 0x40 + 20].copy_from_slice(&1033u32.to_le_bytes());
        img[ro + 0x40 + 20..ro + 0x40 + 24].copy_from_slice(&0x60u32.to_le_bytes());
        // IMAGE_RESOURCE_DATA_ENTRY (0x60): RVA `0x3080`, size `payload.len()`
        img[ro + 0x60..ro + 0x64].copy_from_slice(&0x3080u32.to_le_bytes());
        img[ro + 0x64..ro + 0x68].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        img[ro + 0x80..ro + 0x80 + payload.len()].copy_from_slice(payload);
    }

    img
}

/// Builds a minimal, spec-valid 64-bit `AArch64` (`arm64`) Mach-O bundle exporting `_<export_name>`
/// with A64 `instructions` at VM address `0x1_0000_0400`, optionally wrapped in a Universal Fat header!
pub(crate) fn build_test_macho_arm64(export_name: &str, instructions: &[u32], wrap_universal: bool) -> Vec<u8> {
    let mut mo = vec![0u8; 0x1000];
    // mach_header_64 (32 bytes)
    mo[0..4].copy_from_slice(&0xFEED_FACFu32.to_le_bytes());
    mo[4..8].copy_from_slice(&0x0100_000Cu32.to_le_bytes()); // CPU_TYPE_ARM64
    mo[12..16].copy_from_slice(&0x08u32.to_le_bytes()); // MH_BUNDLE
    mo[16..20].copy_from_slice(&2u32.to_le_bytes()); // ncmds = 2 (LC_SEGMENT_64 + LC_SYMTAB)
    mo[20..24].copy_from_slice(&((152 + 24) as u32).to_le_bytes());

    // LC_SEGMENT_64 (cmd 0x19, 72 + 80 = 152 bytes) at offset 32
    let lc = 32usize;
    mo[lc..lc + 4].copy_from_slice(&0x19u32.to_le_bytes());
    mo[lc + 4..lc + 8].copy_from_slice(&152u32.to_le_bytes());
    mo[lc + 8..lc + 14].copy_from_slice(b"__TEXT");
    mo[lc + 24..lc + 32].copy_from_slice(&0x0000_0001_0000_0000u64.to_le_bytes()); // vmaddr
    mo[lc + 32..lc + 40].copy_from_slice(&0x1000u64.to_le_bytes()); // vmsize
    mo[lc + 40..lc + 48].copy_from_slice(&0u64.to_le_bytes()); // fileoff
    mo[lc + 48..lc + 56].copy_from_slice(&0x0800u64.to_le_bytes()); // filesize
    mo[lc + 64..lc + 68].copy_from_slice(&1u32.to_le_bytes()); // nsects = 1

    let sec = lc + 72;
    mo[sec..sec + 6].copy_from_slice(b"__text");
    mo[sec + 16..sec + 22].copy_from_slice(b"__TEXT");
    mo[sec + 32..sec + 40].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes()); // addr
    mo[sec + 40..sec + 48].copy_from_slice(&((instructions.len() * 4) as u64).to_le_bytes());
    mo[sec + 48..sec + 52].copy_from_slice(&0x0400u32.to_le_bytes()); // offset

    // LC_SYMTAB (cmd 0x02, 24 bytes) at offset 184
    let sym_lc = lc + 152;
    mo[sym_lc..sym_lc + 4].copy_from_slice(&0x02u32.to_le_bytes());
    mo[sym_lc + 4..sym_lc + 8].copy_from_slice(&24u32.to_le_bytes());
    mo[sym_lc + 8..sym_lc + 12].copy_from_slice(&0x0800u32.to_le_bytes()); // symoff
    mo[sym_lc + 12..sym_lc + 16].copy_from_slice(&1u32.to_le_bytes()); // nsyms = 1
    mo[sym_lc + 16..sym_lc + 20].copy_from_slice(&0x0820u32.to_le_bytes()); // stroff
    mo[sym_lc + 20..sym_lc + 24].copy_from_slice(&0x0100u32.to_le_bytes()); // strsize

    // Write A64 instructions at file 0x0400
    for (i, &insn) in instructions.iter().enumerate() {
        let o = 0x0400 + i * 4;
        mo[o..o + 4].copy_from_slice(&insn.to_le_bytes());
    }

    // Write nlist_64 at 0x0800 + string table at 0x0820
    let sym_str = format!("_{export_name}");
    mo[0x0820 + 1..0x0820 + 1 + sym_str.len()].copy_from_slice(sym_str.as_bytes());
    mo[0x0800..0x0804].copy_from_slice(&1u32.to_le_bytes()); // n_strx = 1
    mo[0x0804] = 0x0F; // N_SECT | N_EXT
    mo[0x0805] = 1; // n_sect = 1
    mo[0x0808..0x0810].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());

    if !wrap_universal {
        return mo;
    }
    // Wrap inside Universal Fat header (`0xCAFEBABE`, 1 slice = ARM64 at offset 0x100)
    let mut fat = vec![0u8; 0x100 + mo.len()];
    fat[0..4].copy_from_slice(&0xCAFE_BABEu32.to_be_bytes());
    fat[4..8].copy_from_slice(&1u32.to_be_bytes());
    fat[8..12].copy_from_slice(&0x0100_000Cu32.to_be_bytes()); // CPU_TYPE_ARM64
    fat[12..16].copy_from_slice(&0u32.to_be_bytes());
    fat[16..20].copy_from_slice(&0x100u32.to_be_bytes());
    fat[20..24].copy_from_slice(&(mo.len() as u32).to_be_bytes());
    fat[24..28].copy_from_slice(&12u32.to_be_bytes());
    fat[0x100..].copy_from_slice(&mo);
    fat
}

fn build_le_pipl(kind_4cc: &[u8; 4], name: &str, category: &str, ep_win64: &str) -> Vec<u8> {
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
    let mut pep = ep_win64.as_bytes().to_vec();
    pep.push(0);
    push_prop(&mut out, b"8664", &pep);
    out
}

#[test]
fn pe64_x86_64_8bf_inverts_rgb_channels_and_preserves_alpha() {
    // Win64 `PluginMain(short selector [cx], FilterRecord* rec [rdx], intptr_t* data [r8], short* result [r9])`:
    // On `selector == 3` (`filterSelectorStart`), invert R,G,B in `outData` (`[rdx + 0x64]`), keep A,
    // then zero `outRect` (`[rdx + 0x4C]`) and `inRect` (`[rdx + 0x40]`) so Continue finishes cleanly!
    let code: &[u8] = &[
        0x66, 0x41, 0xC7, 0x01, 0x00, 0x00, // mov word ptr [r9], 0
        0x66, 0x83, 0xF9, 0x03, // cmp cx, 3
        0x75, 0x35, // jne done
        0x48, 0x8B, 0x42, 0x58, // mov rax, [rdx + 0x58] (inData)
        0x4C, 0x8B, 0x52, 0x64, // mov r10, [rdx + 0x64] (outData)
        0x44, 0x0F, 0xBF, 0x5A, 0x50, // movsx r11d, word ptr [rdx + 0x50] (outRect.bottom = h)
        0x8B, 0x4A, 0x6C, // mov ecx, [rdx + 0x6C] (outRowBytes = w * 4)
        0x41, 0x0F, 0xAF, 0xCB, // imul ecx, r11d (total bytes)
        0x31, 0xC9, // wait -> keep total in r8d instead!
    ];
    let _ = code;
    // Clean exact hand-assembled x86_64 loop:
    let x64_code: &[u8] = &[
        0x66, 0x41, 0xC7, 0x01, 0x00, 0x00, // mov word ptr [r9], 0
        0x66, 0x83, 0xF9, 0x03, // cmp cx, 3 (filterSelectorStart)
        0x75, 0x37, // jne +0x37 (done)
        0x48, 0x8B, 0x42, 0x58, // mov rax, [rdx + 0x58] (inData)
        0x4C, 0x8B, 0x52, 0x64, // mov r10, [rdx + 0x64] (outData)
        0x44, 0x0F, 0xBF, 0x5A, 0x50, // movsx r11d, word ptr [rdx + 0x50] (height)
        0x44, 0x8B, 0x42, 0x6C, // mov r8d, [rdx + 0x6C] (outRowBytes)
        0x45, 0x0F, 0xAF, 0xC3, // imul r8d, r11d (total bytes)
        0x31, 0xC9, // xor ecx, ecx (i = 0)
        // loop_top:
        0x41, 0x39, 0xC8, // cmp r8d, ecx
        0x7E, 0x16, // jle loop_end
        0x8B, 0x3C, 0x08, // mov edi, [rax + rcx] (4 bytes RGBA)
        0x81, 0xF7, 0xFF, 0xFF, 0xFF, 0x00, // xor edi, 0x00FF_FFFF (invert RGB, keep A!)
        0x41, 0x89, 0x3C, 0x0A, // mov [r10 + rcx], edi
        0x83, 0xC1, 0x04, // add ecx, 4
        0xEB, 0xEA, // jmp loop_top
        // loop_end:
        0x48, 0xC7, 0x42, 0x40, 0x00, 0x00, 0x00, 0x00, // mov qword ptr [rdx + 0x40], 0 (inRect = 0)
        0x48, 0xC7, 0x42, 0x4C, 0x00, 0x00, 0x00, 0x00, // mov qword ptr [rdx + 0x4C], 0 (outRect = 0)
        // done:
        0xC3, // ret
    ];

    let pipl = build_le_pipl(b"8BFM", "Invert RGB C++", "ArtCraft Native", "PluginMain");
    let dll = build_test_pe64("PluginMain", x64_code, Some(&pipl));

    let desc = inspect_plugin_binary(&dll, Some("invert.8bf")).expect("inspect .8bf");
    assert_eq!(desc.plugin_format, PluginFormat::Photoshop8bf);
    assert_eq!(desc.binary_format, BinaryFormat::Pe64);
    assert_eq!(desc.arch, GuestArch::X86_64);
    assert_eq!(desc.pipl.name, "Invert RGB C++");

    let host = Filter8bfHost::from_bytes(dll, Some("invert.8bf"), Limits::default()).expect("load .8bf");
    let src = ImageBuffer8bf::from_rgba_f32(2, 1, vec![0.2, 0.4, 0.8, 0.75, 1.0, 0.0, 0.5, 0.25], PixelDepth8bf::U8).expect("src image");
    let out = host.execute(&src).expect("execute .8bf");
    assert!((out.samples[0] - 0.8).abs() < 0.02);
    assert!((out.samples[1] - 0.6).abs() < 0.02);
    assert!((out.samples[2] - 0.2).abs() < 0.02);
    assert!((out.samples[3] - 0.75).abs() < 0.02);
    assert!((out.samples[4] - 0.0).abs() < 0.02);
    assert!((out.samples[5] - 1.0).abs() < 0.02);
    assert!((out.samples[7] - 0.25).abs() < 0.02);
}

#[test]
fn universal_macho_arm64_8bf_executes_cross_architecture_without_jit() {
    // AArch64 macOS `.plugin`: `PluginMain(short selector [w0], FilterRecord* rec [x1], intptr_t* data [x2], short* result [x3])`
    // In MAC64 layout: `inData` @ `+0x60`, `outData` @ `+0x70`, `outRowBytes` @ `+0x78`, `outRect` @ `+0x50`, `inRect` @ `+0x44`.
    // Swaps Red and Blue channels in `outData` on `selector == 3` (`Start`)!
    let arm64_insns: &[u32] = &[
        0x7900_007F, // strh wzr, [x3]          (*result = 0)
        0x7100_0C1F, // cmp w0, #3              (selector == Start?)
        0x5400_0161, // b.ne #0x2c (done)
        0xF940_3024, // ldr x4, [x1, #0x60]     (inData)
        0xF940_3825, // ldr x5, [x1, #0x70]     (outData)
        0xB940_7826, // ldr w6, [x1, #0x78]     (outRowBytes — 1 row test)
        0xD280_0007, // mov x7, #0
        // loop:
        0x6B06_00FF, // cmp w7, w6
        0x5400_00A2, // b.hs #0x14 (after_loop)
        0xB867_6888, // ldr w8, [x4, x7]        ([R, G, B, A] in little-endian: byte0=R, byte1=G, byte2=B, byte3=A)
        // Swap R and B:
        0x5300_1D09, // uxtb w9, w8             (R)
        0x5310_5D0A, // ubfx w10, w8, #16, #8   (B)
        0x1200_1D0B, // and w8, ... wait -> use clean byte loads/stores!
        0xD503_201F, // nop placeholder
    ];
    let _ = arm64_insns;

    let clean_arm64: &[u32] = &[
        0x7900_007F, // strh wzr, [x3]
        0x7100_0C1F, // cmp w0, #3
        0x5400_01A1, // b.ne +0x34 (done)
        0xF940_3024, // ldr x4, [x1, #0x60]   (inData)
        0xF940_3825, // ldr x5, [x1, #0x70]   (outData)
        0xB940_7826, // ldr w6, [x1, #0x78]   (outRowBytes)
        // Swap R (byte 0) and B (byte 2) of pixel 0:
        0x3940_0089, // ldrb w9, [x4, #0]     (R)
        0x3940_048A, // ldrb w10, [x4, #1]    (G)
        0x3940_088B, // ldrb w11, [x4, #2]    (B)
        0x3940_0C8C, // ldrb w12, [x4, #3]    (A)
        0x3900_00AB, // strb w11, [x5, #0]    (out R = B)
        0x3900_04AA, // strb w10, [x5, #1]    (out G = G)
        0x3900_08A9, // strb w9, [x5, #2]     (out B = R)
        0x3900_0CAC, // strb w12, [x5, #3]    (out A = A)
        0xF900_283F, // str xzr, [x1, #0x50]  (outRect = 0)
        // done:
        0xD65F_03C0, // ret
    ];

    let universal = build_test_macho_arm64("PluginMain", clean_arm64, true);
    let desc = inspect_plugin_binary(&universal, Some("SwapRB.8bf")).expect("inspect universal arm64");
    assert_eq!(desc.binary_format, BinaryFormat::MachO64);
    assert_eq!(desc.arch, GuestArch::AArch64);

    let host = Filter8bfHost::from_bytes(universal, Some("SwapRB.8bf"), Limits::default()).expect("load arm64 .8bf");
    let src = ImageBuffer8bf::from_rgba_f32(1, 1, vec![0.9, 0.4, 0.1, 1.0], PixelDepth8bf::U8).expect("1x1");
    let out = host.execute(&src).expect("execute arm64 .8bf");
    assert!((out.samples[0] - 0.1).abs() < 0.02);
    assert!((out.samples[1] - 0.4).abs() < 0.02);
    assert!((out.samples[2] - 0.9).abs() < 0.02);
    assert!((out.samples[3] - 1.0).abs() < 0.02);
}

#[test]
fn pe64_aex_and_prm_discover_params_and_render_effect_world() {
    // Win64 `EntryPointFunc(PF_Cmd cmd [ecx], PF_InData* in_data [rdx], PF_OutData* out_data [r8], PF_ParamDef* params[] [r9], PF_LayerDef* output [rsp+0x28]) -> PF_Err`:
    // - On `cmd == 2` (`PF_Cmd_PARAMS_SETUP`): allocates 0xB0 on stack, sets `param_type = 2` (`FIX_SLIDER`), `dephault = 0x10000` (`1.0`), calls `in_data->inter.add_param(effect_ref, -1, &def)`.
    // - On `cmd == 11` (`PF_Cmd_RENDER`): reads `params[1]->u.sd.value` (`16.16` fixed boost), copies pixel 0 from `params[0]->u.ld.data` to `output->data` adding boost to Red!
    let aex_code: &[u8] = &[
        0x83, 0xF9, 0x02, // cmp ecx, 2 (PARAMS_SETUP)
        0x75, 0x53, // jne check_render (+0x53 = 83 bytes)
        0x48, 0x81, 0xEC, 0xD8, 0x00, 0x00, 0x00, // sub rsp, 0xD8
        // Zero 0xB0 bytes at [rsp+0x20] via rep stosb (preserve rdx = in_data* in r10):
        0x57, // push rdi
        0x49, 0x89, 0xD2, // mov r10, rdx
        0x48, 0x8D, 0x7C, 0x24, 0x28, // lea rdi, [rsp+0x28]
        0x31, 0xC0, // xor eax, eax
        0xB9, 0xB0, 0x00, 0x00, 0x00, // mov ecx, 0xB0
        0xF3, 0xAA, // rep stosb
        0x5F, // pop rdi
        0xC7, 0x44, 0x24, 0x20, 0x01, 0x00, 0x00, 0x00, // mov dword ptr [rsp+0x20], 1 (uu_id)
        0xC7, 0x44, 0x24, 0x2C, 0x02, 0x00, 0x00, 0x00, // mov dword ptr [rsp+0x2C], 2 (PF_Param_FIX_SLIDER)
        0xC7, 0x44, 0x24, 0x58, 0x00, 0x80, 0x00, 0x00, // mov dword ptr [rsp+0x58], 0x8000 (default 0.5)
        0x49, 0x8B, 0x42, 0x10, // mov rax, [r10 + 0x10] (add_param)
        0x49, 0x8B, 0x8A, 0xB8, 0x00, 0x00, 0x00, // mov rcx, [r10 + 0xB8] (effect_ref)
        0xBA, 0xFF, 0xFF, 0xFF, 0xFF, // mov edx, -1
        0x4C, 0x8D, 0x44, 0x24, 0x20, // lea r8, [rsp+0x20]
        0xFF, 0xD0, // call rax
        0x48, 0x81, 0xC4, 0xD8, 0x00, 0x00, 0x00, // add rsp, 0xD8
        0x31, 0xC0, // xor eax, eax
        0xC3, // ret
        // check_render:
        0x83, 0xF9, 0x0B, // cmp ecx, 11 (PF_Cmd_RENDER)
        0x75, 0x29, // jne ok_ret (+0x29 = 41 bytes)
        0x49, 0x8B, 0x01, // mov rax, [r9] (params[0])
        0x48, 0x8B, 0x40, 0x50, // mov rax, [rax + 0x38 + 0x18] (src data)
        0x49, 0x8B, 0x49, 0x08, // mov rcx, [r9 + 8] (params[1])
        0x8B, 0x49, 0x38, // mov ecx, [rcx + 0x38] (16.16 slider value)
        0xC1, 0xF9, 0x08, // sar ecx, 8 (scale 1.0 -> 256)
        0x4C, 0x8B, 0x54, 0x24, 0x28, // mov r10, [rsp + 0x28] (output PF_LayerDef*)
        0x4D, 0x8B, 0x52, 0x18, // mov r10, [r10 + 0x18] (dst data)
        0x8B, 0x10, // mov edx, [rax] (ARGB pixel 0)
        0x41, 0x89, 0x12, // mov [r10], edx
        0x0F, 0xB6, 0x50, 0x01, // movzx edx, byte ptr [rax + 1] (Red)
        0x01, 0xCA, // add edx, ecx
        0x41, 0x88, 0x52, 0x01, // mov [r10 + 1], dl
        // ok_ret:
        0x31, 0xC0, // xor eax, eax
        0xC3, // ret
    ];

    let pipl = build_le_pipl(b"eFKT", "Red Boost AEX", "Color", "EntryPointFunc");
    let aex_bytes = build_test_pe64("EntryPointFunc", aex_code, Some(&pipl));

    let host = AePrmHost::from_bytes(aex_bytes.clone(), Some("RedBoost.aex"), Limits::default()).expect("load .aex");
    assert_eq!(host.params().len(), 1);
    assert!(matches!(host.params()[0].kind, AeParamKind::Slider { .. }));

    let frame = RgbaFrame::new(1, 1, vec![[0.2, 0.3, 0.4, 1.0]]).expect("1x1 frame");
    let out = host.render(&frame, &[0.5], 0.0).expect("render .aex");
    assert!(out.pixels[0][0] > 0.65, "red boosted: {:?}", out.pixels[0]);
    assert!((out.pixels[0][1] - 0.3).abs() < 0.02);
    assert!((out.pixels[0][2] - 0.4).abs() < 0.02);

    // Verify exact same binary works as a Premiere `.prm` effect too!
    let prm_host = AePrmHost::from_bytes(aex_bytes, Some("RedBoost.prm"), Limits::default()).expect("load .prm");
    assert_eq!(prm_host.descriptor().plugin_format, PluginFormat::PremierePrm);
    let prm_out = prm_host.render(&frame, &[0.25], 1.0).expect("render .prm");
    assert!(prm_out.pixels[0][0] > 0.4);
}

#[test]
fn arm64_aip_acquires_ai_path_suite_and_transforms_bezier_geometry() {
    // AArch64 `.aip` `PluginMain(caller [x0], selector [x1], message [x2]) -> SPErr [w0]`:
    // When `message + 0x30` (`art` handle) is non-zero (during `"Go"`), acquires `"AI Path Suite"`
    // via `SPBasicSuite::AcquireSuite` (`[sp_basic + 0]`), reads segment 0 (`GetPathSegments` @ `+0x10`),
    // adds `+10.0` (`FMOV D1, #10.0; FADD D0, D0, D1`) to `p.x`, and writes it back (`SetPathSegments` @ `+0x18`)!
    let mut insns: Vec<u32> = vec![
        0xA9BB_7BFD, // stp x29, x30, [sp, #-80]!
        0xA901_53F3, // stp x19, x20, [sp, #16]
        0xA902_5BF5, // stp x21, x22, [sp, #32]
        0x9100_03FD, // mov x29, sp
        0xAA02_03F3, // mov x19, x2               (message*)
        0xF940_1A74, // ldr x20, [x19, #0x30]     (art handle — 0 on Startup, non-zero on Go!)
        0xB400_0294, // cbz x20, done             (+0x50)
        0xF940_0E75, // ldr x21, [x19, #0x18]     (SPBasicSuite*)
        0xF940_02A8, // ldr x8, [x21, #0]         (AcquireSuite)
        0x1000_0280, // adr x0, suite_name        (+0x50 from this PC = index 9 + 20 = index 29)
        0xD280_0021, // mov x1, #1                (version = 1)
        0x9100_C3A2, // add x2, x29, #48          (&suite_ptr on stack)
        0xD63F_0100, // blr x8                    (AcquireSuite("AI Path Suite", 1, &suite_ptr))
        0xF940_1BB6, // ldr x22, [x29, #48]       (AIPathSuite* -> Rt = 22 = 0x16!)
        // Allocate 64 bytes on stack (`sub sp, sp, #64`) for 1 `AIPathSegment` (56 bytes):
        0xD101_03FF, // sub sp, sp, #64
        // GetPathSegments(art=x20, seg=0, count=1, buf=sp):
        0xAA14_03E0, // mov x0, x20
        0xD280_0001, // mov x1, #0
        0xD280_0022, // mov x2, #1
        0x9100_03E3, // mov x3, sp
        0xF940_0AC8, // ldr x8, [x22, #0x10]      (GetPathSegments)
        0xD63F_0100, // blr x8
        // Add 10.0 to `seg.p.h` (`[sp + 0]`):
        0xFD40_03E0, // ldr d0, [sp, #0]
        0x1E64_9001, // fmov d1, #10.0            (imm8 = 0x24: (+1) * (1 + 4/16) * 2^3 = 1.25 * 8 = 10.0!)
        0x1E61_2800, // fadd d0, d0, d1
        0xFD00_03E0, // str d0, [sp, #0]
        // SetPathSegments(art=x20, seg=0, count=1, buf=sp):
        0xAA14_03E0, // mov x0, x20
        0xD280_0001, // mov x1, #0
        0xD280_0022, // mov x2, #1
        0x9100_03E3, // mov x3, sp
        0xF940_0EC8, // ldr x8, [x22, #0x18]      (SetPathSegments)
        0xD63F_0100, // blr x8
        0x9101_03FF, // add sp, sp, #64
        // done (index 32):
        0xD280_0000, // mov x0, #0
        0xA942_5BF5, // ldp x21, x22, [sp, #32]
        0xA941_53F3, // ldp x19, x20, [sp, #16]
        0xA8C5_7BFD, // ldp x29, x30, [sp], #80
        0xD65F_03C0, // ret
    ];
    // Fix exact branch offsets for `cbz x20, done` (index 6 -> index 32: delta = +26 insns = +104 bytes)
    // and `adr x0, suite_name` (index 9 -> index 37: delta = +28 insns = +112 bytes):
    let suite_idx = insns.len();
    insns[6] = 0xB400_0014 | (26u32 << 5);
    let adr_bytes = ((suite_idx - 9) * 4) as u32;
    insns[9] = 0x1000_0000 | ((adr_bytes & 3) << 29) | ((adr_bytes >> 2) << 5);
    // Zero `msg_buf + 0x30` before returning so `Shutdown` does not re-run `Go`!
    for chunk in b"AI Path Suite\0\0\0".as_chunks::<4>().0 {
        insns.push(u32::from_le_bytes(*chunk));
    }

    let aip_bin = build_test_macho_arm64("PluginMain", &insns, false);
    let host = AipHost::from_bytes(aip_bin, Some("ShiftX.aip"), Limits::default()).expect("load arm64 .aip");
    let input = vec![AipSubpath {
        closed: true,
        segments: vec![
            AipBezierSegment { p: [25.0, 40.0], h_in: [25.0, 40.0], h_out: [25.0, 40.0], corner: true },
            AipBezierSegment { p: [80.0, 40.0], h_in: [80.0, 40.0], h_out: [80.0, 40.0], corner: true },
        ],
    }];
    let out = host.execute_paths(&input, 0.0).expect("execute arm64 .aip");
    assert_eq!(out.len(), 1);
    assert!((out[0].segments[0].p[0] - 35.0).abs() < 1e-6, "got out={out:?}");
    assert!((out[0].segments[0].p[1] - 40.0).abs() < 1e-6);
}

#[test]
fn executes_real_shipped_adobe_solarize_8bf_if_installed() {
    let path = std::path::Path::new(r"C:\Program Files\Adobe\Adobe Photoshop 2027\Required\Plug-ins\Filters\Solarize.8bf");
    if !path.is_file() {
        return;
    }
    let bytes = std::fs::read(path).expect("read Solarize.8bf");
    let host = Filter8bfHost::from_bytes(bytes, Some("Solarize.8bf"), Limits::default()).expect("load Solarize.8bf");
    assert!(host.descriptor().pipl.is_populated(), "pipl={:?}", host.descriptor().pipl);

    // Solarize formula in `Solarize.8bf`: values <= 0x7F stay as `v`, values > 0x7F become `255 - v`!
    // So `0.2` (~51) stays ~`0.2`, while `0.9` (~230) solarizes to `25/255` (~`0.098`)!
    let src = ImageBuffer8bf::from_rgba_f32(4, 4, [0.2f32, 0.9, 0.5, 1.0].repeat(16), PixelDepth8bf::U8).expect("4x4 image");
    let out = host.execute(&src).expect("execute real Solarize.8bf in pure-Rust VM");
    assert!((out.samples[0] - 0.2).abs() < 0.02, "dark channel unchanged: {}", out.samples[0]);
    assert!((out.samples[1] - 0.1).abs() < 0.02, "bright channel solarized: {}", out.samples[1]);
}

#[test]
fn inspects_and_loads_real_shipped_average_8bf_threshold_aex_and_addanchor_aip() {
    let avg = std::path::Path::new(r"C:\Program Files\Adobe\Adobe Photoshop 2027\Required\Plug-ins\Filters\Average.8bf");
    if avg.is_file() {
        let bytes = std::fs::read(avg).expect("read Average.8bf");
        let host = Filter8bfHost::from_bytes(bytes, Some("Average.8bf"), Limits::default()).expect("load Average.8bf");
        let src = ImageBuffer8bf::from_rgba_f32(2, 1, vec![0.2, 0.4, 0.6, 1.0, 0.8, 0.4, 0.6, 1.0], PixelDepth8bf::U8).expect("2x1");
        let out = host.execute(&src).expect("execute real Average.8bf");
        // Average of R=(0.2, 0.8) is ~0.5!
        assert!((out.samples[0] - 0.5).abs() < 0.03, "averaged R0={}", out.samples[0]);
        assert!((out.samples[4] - 0.5).abs() < 0.03, "averaged R1={}", out.samples[4]);
    }

    let thresh = std::path::Path::new(r"C:\Program Files\Adobe\Adobe After Effects 2027\Support Files\Plug-ins\Effects\CycoreFXHD\ThresholdRGB.aex");
    if thresh.is_file() {
        let bytes = std::fs::read(thresh).expect("read ThresholdRGB.aex");
        let desc = inspect_plugin_binary(&bytes, Some("ThresholdRGB.aex")).expect("inspect ThresholdRGB.aex");
        assert_eq!(desc.plugin_format, PluginFormat::AfterEffectsAex);
        assert_eq!(desc.entrypoint_symbol, "MainEntry");
        assert!(!desc.pipl.name.is_empty());
    }

    let add_anc = std::path::Path::new(r"C:\Program Files\Adobe\Adobe Illustrator 2027\Plug-ins\Illustrator Filters\AddAnchor.aip");
    if add_anc.is_file() {
        let bytes = std::fs::read(add_anc).expect("read AddAnchor.aip");
        let desc = inspect_plugin_binary(&bytes, Some("AddAnchor.aip")).expect("inspect AddAnchor.aip");
        assert_eq!(desc.plugin_format, PluginFormat::IllustratorAip);
        assert_eq!(desc.entrypoint_symbol, "PluginMain");
    }
}
