//! Engine integration tests for `plugin.install`, `plugin.run`, `effect.apply`, and plug-in store
//! persistence with compiled C++ `.aip` binaries.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;
use vectorcraft_plugins::registry;

fn minimal_arm64_shift_aip(live_effect: bool) -> Vec<u8> {
    // Minimal AArch64 Mach-O `.aip` (`PluginMain`) shifting segment 0 `p.x` by +10.0 on `"Go"`,
    // and registering in `AILiveEffectSuite` when `live_effect` is true.
    let mut insns: Vec<u32> = vec![
        0xA9BB_7BFD, // stp x29, x30, [sp, #-80]!
        0xA901_53F3, // stp x19, x20, [sp, #16]
        0xA902_5BF5, // stp x21, x22, [sp, #32]
        0x9100_03FD, // mov x29, sp
        0xAA02_03F3, // mov x19, x2 (message*)
        0xF940_1A74, // ldr x20, [x19, #0x30] (art handle)
        0xB400_0000, // [6] cbz x20, startup_or_done
        0xF940_0E75, // ldr x21, [x19, #0x18] (SPBasicSuite*)
        0xF940_02A8, // ldr x8, [x21, #0]
        0x1000_0000, // [9] adr x0, str_path_suite
        0xD280_0021, // mov x1, #1
        0x9100_C3A2, // add x2, x29, #48
        0xD63F_0100, // blr x8
        0xF940_1BB6, // ldr x22, [x29, #48]
        0xD101_03FF, // sub sp, sp, #64
        0xAA14_03E0, // mov x0, x20
        0xD280_0001, // mov x1, #0
        0xD280_0022, // mov x2, #1
        0x9100_03E3, // mov x3, sp
        0xF940_0AC8, // ldr x8, [x22, #0x10]
        0xD63F_0100, // blr x8
        0xFD40_03E0, // ldr d0, [sp, #0]
        0x1E64_9001, // fmov d1, #10.0
        0x1E61_2800, // fadd d0, d0, d1
        0xFD00_03E0, // str d0, [sp, #0]
        0xAA14_03E0, // mov x0, x20
        0xD280_0001, // mov x1, #0
        0xD280_0022, // mov x2, #1
        0x9100_03E3, // mov x3, sp
        0xF940_0EC8, // ldr x8, [x22, #0x18]
        0xD63F_0100, // blr x8
        0x9101_03FF, // add sp, sp, #64
    ];

    let mut patch_b_done = None;
    let mut patch_startup_bne = None;
    let mut patch_adr_live = None;
    if live_effect {
        let idx_b = insns.len();
        insns.push(0); // b done
        patch_b_done = Some(idx_b);
    }

    let startup_label = insns.len();
    if live_effect {
        insns.push(0x3940_0828); // ldrb w8, [x1, #2] ('a' in "Startup")
        insns.push(0x7101_851F); // cmp w8, #0x61
        let idx_bne = insns.len();
        insns.push(0); // b.ne done
        patch_startup_bne = Some(idx_bne);
        insns.push(0xF940_0E75); // ldr x21, [x19, #0x18]
        let idx_adr = insns.len();
        insns.push(0); // adr x0, str_live_suite
        patch_adr_live = Some(idx_adr);
        insns.push(0xD280_0021); // mov x1, #1
        insns.push(0x9100_C3A2); // add x2, x29, #48
        insns.push(0xF940_02A8); // ldr x8, [x21, #0]
        insns.push(0xD63F_0100); // blr x8
        insns.push(0xF940_1BB6); // ldr x22, [x29, #48]
        insns.push(0xD280_0000); // mov x0, #0
        insns.push(0x9100_E3A1); // add x1, x29, #56
        insns.push(0xD280_0002); // mov x2, #0
        insns.push(0xD280_0003); // mov x3, #0
        insns.push(0xF940_02C8); // ldr x8, [x22, #0]
        insns.push(0xD63F_0100); // blr x8
    }

    let done = insns.len();
    insns.push(0xD280_0000); // mov x0, #0
    insns.push(0xA942_5BF5); // ldp x21, x22, [sp, #32]
    insns.push(0xA941_53F3); // ldp x19, x20, [sp, #16]
    insns.push(0xA8C5_7BFD); // ldp x29, x30, [sp], #80
    insns.push(0xD65F_03C0); // ret

    let path_str_idx = insns.len();
    for chunk in b"AI Path Suite\0\0\0".as_chunks::<4>().0 {
        insns.push(u32::from_le_bytes(*chunk));
    }
    let live_str_idx = insns.len();
    for chunk in b"AI Live Effect Suite\0\0\0\0".as_chunks::<4>().0 {
        insns.push(u32::from_le_bytes(*chunk));
    }

    let cbz_target = if live_effect { startup_label } else { done };
    insns[6] = 0xB400_0014 | (((cbz_target - 6) as u32) << 5);
    let adr_bytes = ((path_str_idx - 9) * 4) as u32;
    insns[9] = 0x1000_0000 | ((adr_bytes & 3) << 29) | ((adr_bytes >> 2) << 5);
    if let Some(i) = patch_b_done {
        insns[i] = 0x1400_0000 | (((done - i) as u32) & 0x03FF_FFFF);
    }
    if let Some(i) = patch_startup_bne {
        insns[i] = 0x5400_0001 | (((done - i) as u32) << 5);
    }
    if let Some(i) = patch_adr_live {
        let b = ((live_str_idx - i) * 4) as u32;
        insns[i] = 0x1000_0000 | ((b & 3) << 29) | ((b >> 2) << 5);
    }

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
    mo[lc + 48..lc + 56].copy_from_slice(&0x0800u64.to_le_bytes());
    mo[lc + 64..lc + 68].copy_from_slice(&1u32.to_le_bytes());
    let sec = lc + 72;
    mo[sec..sec + 6].copy_from_slice(b"__text");
    mo[sec + 16..sec + 22].copy_from_slice(b"__TEXT");
    mo[sec + 32..sec + 40].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());
    mo[sec + 40..sec + 48].copy_from_slice(&((insns.len() * 4) as u64).to_le_bytes());
    mo[sec + 48..sec + 52].copy_from_slice(&0x0400u32.to_le_bytes());
    let sym_lc = lc + 152;
    mo[sym_lc..sym_lc + 4].copy_from_slice(&0x02u32.to_le_bytes());
    mo[sym_lc + 4..sym_lc + 8].copy_from_slice(&24u32.to_le_bytes());
    mo[sym_lc + 8..sym_lc + 12].copy_from_slice(&0x0800u32.to_le_bytes());
    mo[sym_lc + 12..sym_lc + 16].copy_from_slice(&1u32.to_le_bytes());
    mo[sym_lc + 16..sym_lc + 20].copy_from_slice(&0x0820u32.to_le_bytes());
    mo[sym_lc + 20..sym_lc + 24].copy_from_slice(&0x0100u32.to_le_bytes());
    for (i, &w) in insns.iter().enumerate() {
        let o = 0x0400 + i * 4;
        mo[o..o + 4].copy_from_slice(&w.to_le_bytes());
    }
    mo[0x0821..0x0821 + 11].copy_from_slice(b"_PluginMain");
    mo[0x0800..0x0804].copy_from_slice(&1u32.to_le_bytes());
    mo[0x0804] = 0x0F;
    mo[0x0805] = 1;
    mo[0x0808..0x0810].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());
    mo
}

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn engine_installs_stores_and_runs_aip_filter_and_live_effect() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    vectorcraft_plugins::safe_mode::set_active(None);

    let store_dir = std::env::temp_dir().join(format!("vc-engine-aip-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store_dir);
    std::fs::create_dir_all(&store_dir).unwrap();

    let aip_path = store_dir.join("external_shift.aip");
    let aip_bytes = minimal_arm64_shift_aip(false);
    std::fs::write(&aip_path, &aip_bytes).unwrap();

    let mut s = Session::new();
    let plugin_store = store_dir.join("plugins");
    s.attach_plugin_store(plugin_store.clone());

    let installed = s.execute("plugin.install", &json!({ "path": aip_path.display().to_string() })).expect("plugin.install .aip");
    let id = installed["id"].as_str().unwrap().to_string();
    assert_eq!(id, "aip.external_shift");
    assert_eq!(installed["kind"], "filter");

    // Verify `.aip` extension is preserved on disk in the plug-in store!
    let stored_files: Vec<_> = std::fs::read_dir(&plugin_store)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "aip"))
        .collect();
    assert_eq!(stored_files.len(), 1, "expected 1 stored .aip file in {plugin_store:?}");

    s.execute("file.new", &json!({ "width": 200, "height": 200 })).unwrap();
    let node_id = s.execute("path.create", &json!({ "d": "M20 40L80 40L80 90Z" })).unwrap()["id"].as_u64().unwrap();
    s.execute("select.all", &json!({})).unwrap();
    s.execute("plugin.run", &json!({ "id": id })).expect("plugin.run .aip");

    let p0 = s.active().unwrap().doc.node(NodeId(node_id)).unwrap().path_data().unwrap().subpaths[0].anchors[0].p;
    assert!((p0.x - 30.0).abs() < 1e-6 && (p0.y - 40.0).abs() < 1e-6, "p0={p0:?}");

    // Also install and apply a live-effect `.aip` via `effect.apply`
    let fx_bytes = minimal_arm64_shift_aip(true);
    let fx_b64 = vectorcraft_format::base64_encode(&fx_bytes);
    let fx_inst = s.execute("plugin.install", &json!({ "dataBase64": fx_b64, "name": "LiveShift.aip" })).expect("install live effect .aip");
    let fx_id = fx_inst["id"].as_str().unwrap().to_string();
    let fx_effect_id = fx_inst["effect"].as_str().unwrap().to_string();
    assert_eq!(fx_inst["kind"], "effect");
    s.execute("effect.apply", &json!({ "effect": fx_effect_id })).expect("effect.apply .aip");

    s.execute("plugin.remove", &json!({ "id": id })).unwrap();
    s.execute("plugin.remove", &json!({ "id": fx_id })).unwrap();
    let _ = registry::remove(&id);
    let _ = registry::remove(&fx_id);
    let _ = std::fs::remove_dir_all(&store_dir);
}
