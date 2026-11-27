//! Unit tests for cross-platform C++ `.aex`, `.prm`, and Mach-O `.plugin` binary execution
//! inside `effectcraft-plugin` (zero-JIT, works on all targets including `wasm32`).

use effectcraft_effects::{Buf, EffectCtx, EffectEnv, Image};
use effectcraft_project::build::Ids;

use crate::native_vm::{Limits, PluginFormat, inspect_plugin_binary};
use crate::{EffectPlugin, NativeAePrmPlugin, load_native_binary, load_plugin, preview};

fn build_test_pe64(export_name: &str, code: &[u8], pipl_payload: Option<&[u8]>) -> Vec<u8> {
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

fn build_test_macho_arm64(export_name: &str, instructions: &[u32], wrap_universal: bool) -> Vec<u8> {
    let mut mo = vec![0u8; 0x1000];
    mo[0..4].copy_from_slice(&0xFEED_FACFu32.to_le_bytes());
    mo[4..8].copy_from_slice(&0x0100_000Cu32.to_le_bytes());
    mo[12..16].copy_from_slice(&0x08u32.to_le_bytes());
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
    fat[12..16].copy_from_slice(&0u32.to_be_bytes());
    fat[16..20].copy_from_slice(&0x100u32.to_be_bytes());
    fat[20..24].copy_from_slice(&(mo.len() as u32).to_be_bytes());
    fat[24..28].copy_from_slice(&12u32.to_be_bytes());
    fat[0x100..].copy_from_slice(&mo);
    fat
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

/// Clean-room x86_64 PE32+ `EntryPointFunc` that registers a `Boost` slider in `PF_Cmd_PARAMS_SETUP`
/// and adds `(slider_val + current_time_sec * 0.25)` to Red across pixel 0 in `PF_Cmd_RENDER`.
fn animated_red_boost_x64_code() -> Vec<u8> {
    vec![
        0x83, 0xF9, 0x02, // cmp ecx, 2 (PF_Cmd_PARAMS_SETUP)
        0x75, 0x63, // jne check_render (+0x63 = 99 bytes)
        0x48, 0x81, 0xEC, 0xD8, 0x00, 0x00, 0x00, // sub rsp, 0xD8
        0x57, // push rdi
        0x49, 0x89, 0xD2, // mov r10, rdx (in_data*)
        0x48, 0x8D, 0x7C, 0x24, 0x28, // lea rdi, [rsp+0x28]
        0x31, 0xC0, // xor eax, eax
        0xB9, 0xB0, 0x00, 0x00, 0x00, // mov ecx, 0xB0
        0xF3, 0xAA, // rep stosb
        0x5F, // pop rdi
        0xC7, 0x44, 0x24, 0x20, 0x01, 0x00, 0x00, 0x00, // uu_id = 1
        0xC7, 0x44, 0x24, 0x2C, 0x02, 0x00, 0x00, 0x00, // PF_Param_FIX_SLIDER = 2
        // ASCII "Boost\0" at [rsp+0x30] (`def_ptr + 0x10`):
        0xC7, 0x44, 0x24, 0x30, 0x42, 0x6F, 0x6F, 0x73, // "Boos"
        0xC7, 0x44, 0x24, 0x34, 0x74, 0x00, 0x00, 0x00, // "t\0\0\0"
        0xC7, 0x44, 0x24, 0x58, 0x00, 0x40, 0x00, 0x00, // value = 0.25 (0x4000)
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
        0x75, 0x37, // jne ok_ret (+0x37 = 55 bytes)
        0x49, 0x8B, 0x01, // mov rax, [r9] (params[0])
        0x48, 0x8B, 0x40, 0x50, // mov rax, [rax + 0x50] (src data)
        0x49, 0x8B, 0x49, 0x08, // mov rcx, [r9 + 8] (params[1])
        0x8B, 0x49, 0x38, // mov ecx, [rcx + 0x38] (16.16 slider)
        0xC1, 0xF9, 0x08, // sar ecx, 8 (1.0 -> 256)
        0x44, 0x8B, 0x9A, 0xE0, 0x00, 0x00, 0x00, // mov r11d, [rdx + 0xE0] (current_time ms)
        0x41, 0xC1, 0xEB, 0x04, // shr r11d, 4 (~62 per sec)
        0x44, 0x01, 0xD9, // add ecx, r11d
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
    ]
}

fn eval_spec(spec: &effectcraft_effects::EffectSpec, boost: f64, time: f64, rgba: [f32; 4]) -> Image {
    let mut next = 1;
    let g = effectcraft_effects::instantiate(spec, &mut Ids(&mut next), spec.name, [1.0, 1.0]);
    let mut params = effectcraft_effects::flatten_params(&g, &mut |p| p.value.clone());
    params.values.insert("boost".into(), effectcraft_keyframe::Value::Scalar(boost));
    let ctx = EffectCtx { params: &params, time, layer_size: [1.0, 1.0], seed: 1, adjustment: false, env: EffectEnv::default() };
    let buf = Buf { img: Image::filled(1, 1, rgba), offset: [0.0; 2], scale: 1.0 };
    (spec.render)(&ctx, buf).img
}

#[test]
fn pe64_aex_and_prm_discover_params_reuse_global_setup_and_animate_frames() {
    let code = animated_red_boost_x64_code();
    let aex_pipl = build_le_pipl(b"eFKT", "Red Boost Native", "Color", "ADBE RedBoostTest", "EntryPointFunc");
    let aex_bytes = build_test_pe64("EntryPointFunc", &code, Some(&aex_pipl));

    // Direct NativeAePrmPlugin check: GLOBAL_SETUP runs once on load and is reused across frames!
    let direct = NativeAePrmPlugin::new(&aex_bytes, Some("RedBoost.aex")).expect("load NativeAePrmPlugin");
    assert_eq!(direct.manifest().id, "aex.ADBE_RedBoostTest");
    assert_eq!(direct.manifest().params.len(), 1);
    assert_eq!(direct.manifest().params[0].id, "boost");
    assert_eq!(direct.global_setup_count(), 1);

    let (_, preview_frames) = preview(&aex_bytes, 1, 1, &[[0.1, 0.2, 0.3, 1.0]], &[0.0, 1.0]).expect("preview .aex");
    assert_eq!(preview_frames.len(), 2);
    assert!(preview_frames[1][0][0] > preview_frames[0][0][0], "animated time shifts output: {preview_frames:?}");

    // Auto-detected registration via `load_plugin` (.aex) and `load_native_binary` (.prm)
    let aex_spec = load_plugin(&aex_bytes, "RedBoost.aex").expect("register .aex");
    assert_eq!(aex_spec.id, "aex.ADBE_RedBoostTest");
    assert_eq!(aex_spec.params.len(), 1);

    // Semi-transparent premultiplied pixel [0.1, 0.15, 0.2, 0.5] (unpremultiplied R=0.2)
    let f0 = eval_spec(aex_spec, 0.25, 0.0, [0.1, 0.15, 0.2, 0.5]).get(0, 0);
    let f1 = eval_spec(aex_spec, 0.50, 0.0, [0.1, 0.15, 0.2, 0.5]).get(0, 0);
    let f2 = eval_spec(aex_spec, 0.25, 1.0, [0.1, 0.15, 0.2, 0.5]).get(0, 0);
    assert!((f0[3] - 0.5).abs() < 0.02);
    assert!(f1[0] > f0[0] + 0.08, "slider animation increased premultiplied R: f0={f0:?}, f1={f1:?}");
    assert!(f2[0] > f0[0] + 0.08, "timeline time increased premultiplied R: f0={f0:?}, f2={f2:?}");

    let prm_pipl = build_le_pipl(b"PrEf", "Premiere Red Boost", "Video Effects", "PR RedBoostTest", "EntryPointFunc");
    let prm_bytes = build_test_pe64("EntryPointFunc", &code, Some(&prm_pipl));
    let prm_spec = load_native_binary(&prm_bytes, Some("PremiereRedBoost.prm")).expect("register .prm");
    assert_eq!(prm_spec.id, "prm.PR_RedBoostTest");
    let prm_px = eval_spec(prm_spec, 0.4, 0.0, [0.2, 0.3, 0.4, 1.0]).get(0, 0);
    assert!(prm_px[0] > 0.55, "prm rendered: {prm_px:?}");
}

#[test]
fn aarch64_macho_plugin_renders_without_jit() {
    // AAPCS64 `EntryPointFunc(cmd [w0], in_data [x1], out_data [x2], params [x3], output [x4])`:
    // On `PF_Cmd_RENDER` (11), loops over `width * height` pixels and swaps Red and Blue!
    let loop_back = (-11i32 as u32) & 0x7FFFF;
    let arm64_insns: &[u32] = &[
        0x7100_2C1F,                // cmp w0, #11 (PF_Cmd_RENDER)
        0x5400_0001 | (20u32 << 5), // b.ne done (+20 insns)
        0xF940_0069,                // ldr x9, [x3]            (params[0])
        0xF940_2929,                // ldr x9, [x9, #0x50]     (src_world data)
        0xF940_0C8A,                // ldr x10, [x4, #0x18]    (dst_world data)
        0xB940_248B,                // ldr w11, [x4, #0x24]    (width)
        0xB940_288C,                // ldr w12, [x4, #0x28]    (height)
        0x1B0C_7D6B,                // mul w11, w11, w12       (pixel count)
        0x3400_000B | (13u32 << 5), // cbz w11, done (+13 insns)
        // loop_body:
        0x3940_012D,                    // ldrb w13, [x9, #0]      (A)
        0x3940_052E,                    // ldrb w14, [x9, #1]      (R)
        0x3940_092F,                    // ldrb w15, [x9, #2]      (G)
        0x3940_0D30,                    // ldrb w16, [x9, #3]      (B)
        0x3900_014D,                    // strb w13, [x10, #0]     (out A = A)
        0x3900_0550,                    // strb w16, [x10, #1]     (out R = B)
        0x3900_094F,                    // strb w15, [x10, #2]     (out G = G)
        0x3900_0D4E,                    // strb w14, [x10, #3]     (out B = R)
        0x9100_1129,                    // add x9, x9, #4
        0x9100_114A,                    // add x10, x10, #4
        0x7100_056B,                    // subs w11, w11, #1
        0x5400_0001 | (loop_back << 5), // b.ne loop_body
        // done:
        0xD280_0000, // mov x0, #0
        0xD65F_03C0, // ret
    ];

    let macho_universal = build_test_macho_arm64("EntryPointFunc", arm64_insns, true);
    let spec = load_plugin(&macho_universal, "SwapRB_ARM64.plugin").expect("load arm64 Mach-O .plugin");
    assert_eq!(spec.id, "aex.SwapRB_ARM64");

    let mut next = 1;
    let g = effectcraft_effects::instantiate(spec, &mut Ids(&mut next), spec.name, [2.0, 2.0]);
    let params = effectcraft_effects::flatten_params(&g, &mut |p| p.value.clone());
    let ctx = EffectCtx { params: &params, time: 0.0, layer_size: [2.0, 2.0], seed: 1, adjustment: false, env: EffectEnv::default() };
    let buf = Buf { img: Image::filled(2, 2, [0.8, 0.4, 0.1, 1.0]), offset: [0.0; 2], scale: 1.0 };
    let out = (spec.render)(&ctx, buf).img.get(1, 1);
    assert!((out[0] - 0.1).abs() < 0.02, "R swapped with B: {out:?}");
    assert!((out[1] - 0.4).abs() < 0.02, "G preserved: {out:?}");
    assert!((out[2] - 0.8).abs() < 0.02, "B swapped with R: {out:?}");
    assert!((out[3] - 1.0).abs() < 0.02);
}

#[test]
fn inspects_and_loads_real_adobe_threshold_rgb_aex_if_present() {
    let candidates = [
        r"C:\Program Files\Adobe\Adobe After Effects 2026\Support Files\Plug-ins\Effects\CycoreFXHD\ThresholdRGB.aex",
        r"C:\Program Files\Adobe\Adobe After Effects 2027\Support Files\Plug-ins\Effects\CycoreFXHD\ThresholdRGB.aex",
    ];
    for cand in candidates {
        let path = std::path::Path::new(cand);
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(path).expect("read ThresholdRGB.aex");
        let desc = inspect_plugin_binary(&bytes, Some("ThresholdRGB.aex")).expect("inspect ThresholdRGB.aex");
        assert_eq!(desc.plugin_format, PluginFormat::AfterEffectsAex);
        assert!(!desc.entrypoint_symbol.is_empty());

        let plugin = NativeAePrmPlugin::new(&bytes, Some("ThresholdRGB.aex")).expect("load real ThresholdRGB.aex");
        assert!(plugin.manifest().id.starts_with("aex."));
        assert!(!plugin.manifest().name.is_empty());
        let _ = Limits::default();
        break;
    }
}
