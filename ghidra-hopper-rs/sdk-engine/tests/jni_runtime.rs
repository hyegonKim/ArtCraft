use ghidra_hopper_rs::{
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    jni_runtime::{sha256, RuntimeSnapshot},
    macho_objc::MachOImage,
    pcode_sleigh::PcodeOp,
};
use serde_json::{json, Value};
fn p32(b: &mut [u8], o: usize, n: u32) {
    b[o..o + 4].copy_from_slice(&n.to_le_bytes());
}
fn p64(b: &mut [u8], o: usize, n: u64) {
    b[o..o + 8].copy_from_slice(&n.to_le_bytes());
}
fn elf() -> Vec<u8> {
    let mut b = vec![0; 0x400];
    b[..6].copy_from_slice(b"\x7fELF\x02\x01");
    b[18..20].copy_from_slice(&183u16.to_le_bytes());
    p64(&mut b, 32, 64);
    p32(&mut b, 64, 1);
    p32(&mut b, 68, 5);
    b[54..56].copy_from_slice(&56u16.to_le_bytes());
    b[56..58].copy_from_slice(&1u16.to_le_bytes());
    p64(&mut b, 80, 0x1000);
    p64(&mut b, 96, 0x400);
    p64(&mut b, 104, 0x400);
    b
}
fn fixture() -> (Vec<u8>, Vec<u8>, Value) {
    let mut b = elf();
    for (n, w) in [
        0xa9be7bfd, 0xa9014ff4, 0x910003fd, 0xf9400008, 0xaa0103f3, 0x90000001, 0xaa0003f4,
        0x912a2c21, 0xf9401908, 0xd63f0100, 0xb4000120, 0xf9400288, 0xaa0003e1, 0xaa1403e0,
        0xaa1303e2, 0xa9414ff4, 0xf9403903, 0xa8c27bfd, 0xd61f0060, 0xa9414ff4, 0x12800000,
        0xa8c27bfd, 0xd65f03c0,
    ]
    .iter()
    .enumerate()
    {
        p32(&mut b, 0x200 + n * 4, *w);
    }
    let mut art = elf();
    for (n, w) in [0xd503201f, 0xd503201f, 0xd503201f, 0xd65f03c0]
        .iter()
        .enumerate()
    {
        p32(&mut art, 0x200 + n * 4, *w);
        p32(&mut art, 0x240 + n * 4, *w);
    }
    let code = art[0x200..0x210]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let v = json!({
        "schema":1,"source":"android-art-process-private-brk","abi":"arm64-v8a",
        "binary_sha256":sha256(&b),"runtime_sha256":sha256(&art),
        "pid":123,"fingerprint":"synthetic-negative-test",
        "env":"0xb400000080000000","x0":"0xb400000080000000","table":"0x70002000",
        "entry":"0x60001200","branch_pc":"0x60001248","x3":"0x70001240",
        "original_word":"0xd61f0060","restored_word":"0xd61f0060","table_stable":true,
        "target_module":{"path":"/data/fixture.so","base":"0x60000000","offset":"0x1200"},
        "slots":[
            {"index":6,"name":"FindClass","value":"0x70001200","code16":code,
             "module":{"path":"/apex/com.android.art/lib64/libart.so","base":"0x70000000","offset":"0x1200"}},
            {"index":14,"name":"ThrowNew","value":"0x70001240","code16":code,
             "module":{"path":"/apex/com.android.art/lib64/libart.so","base":"0x70000000","offset":"0x1240"}}
        ],
        "calls":{"uninstrumented":0,"instrumented":0,"trap_hits":1,
            "exception_class":"java/io/IOException","message_verified":true},
        "maps":["60001000-60002000 r-xp 00000000 00:00 1 /data/fixture.so",
                "70001000-70002000 r-xp 00000000 00:00 2 /apex/com.android.art/lib64/libart.so",
                "70002000-70003000 r--p 00000000 00:00 2 /apex/com.android.art/lib64/libart.so",
                "80000000-80001000 rw-p 00000000 00:00 0 [anon:JNIEnv]"]
    });
    (b, art, v)
}
fn parse(v: &Value, b: &[u8], art: &[u8]) -> RuntimeSnapshot {
    RuntimeSnapshot::parse(&serde_json::to_vec(v).unwrap(), b, art).unwrap()
}
fn buffer_fixture() -> (Vec<u8>, Vec<u8>, Value) {
    let (mut b, art, mut v) = fixture();
    for (n, w) in [
        0xa9be57feu32,
        0xa9014ff4,
        0xf9400008,
        0xaa0203e1,
        0x2a0403f3,
        0x2a0303f4,
        0xaa0003f5,
        0xf9439908,
        0xd63f0100,
        0xf94002a8,
        0x8b34c001,
        0x93407e62,
        0xa9414ff4,
        0xaa1503e0,
        0xf9439503,
        0xa8c257fe,
        0xd61f0060,
    ]
    .iter()
    .enumerate()
    {
        p32(&mut b, 0x200 + n * 4, *w);
    }
    v["schema"] = json!(2);
    v["terminal_jni_slot"] = json!(229);
    v["binary_sha256"] = json!(sha256(&b));
    v["branch_pc"] = json!("0x60001240");
    v["x1"] = json!("0x80000087");
    v["x2"] = json!("0x1f");
    v["x3"] = json!("0x70001200");
    for (n, index, name) in [
        (0, 229, "NewDirectByteBuffer"),
        (1, 230, "GetDirectBufferAddress"),
    ] {
        v["slots"][n]["index"] = json!(index);
        v["slots"][n]["name"] = json!(name);
    }
    v["calls"] = json!({"uninstrumented":0,"instrumented":0,"trap_hits":1,"buffer_verified":true,
        "source_address":"0x80000080","source_capacity":128,"offset":7,"length":31});
    (b, art, v)
}
#[test]
fn direct_buffer_slots_require_actual_buffer_evidence_and_full_env_provenance() {
    let (b, art, v) = buffer_fixture();
    let s = parse(&v, &b, &art);
    let (mut p, mut f) = analyze(&b, 128);
    assert_eq!(s.resolve(&mut p, &mut f).unwrap(), 2);
    assert!(!f.is_incomplete());
    assert_eq!(f.analysis_work, 17);
    let text = f.render_four_modes(p.segment_at(0x1200).unwrap());
    assert!(text.contains("NewDirectByteBuffer"));
    assert!(text.contains("GetDirectBufferAddress"));
    assert!(text.contains("native return allowed"));
    let mut narrow = b.clone();
    p32(&mut narrow, 0x200 + 7 * 4, 0xb9439908);
    let mut nv = v.clone();
    nv["binary_sha256"] = json!(sha256(&narrow));
    let ns = parse(&nv, &narrow, &art);
    let (mut p, mut f) = analyze(&narrow, 128);
    assert_eq!(ns.resolve(&mut p, &mut f).unwrap(), 0);
    assert!(f.is_incomplete());
}
#[test]
fn direct_buffer_capture_rejects_bad_size_address_slot_or_execution_result() {
    let (b, art, v) = buffer_fixture();
    let edits = [
        ("/calls/buffer_verified", json!(false)),
        ("/calls/source_capacity", json!(12)),
        ("/calls/offset", json!(u64::MAX)),
        ("/calls/length", json!(129)),
        ("/x1", json!("0x80000088")),
        ("/x2", json!("0x20")),
        ("/calls/source_address", json!("0x0")),
        ("/slots/0/index", json!(14)),
        ("/terminal_jni_slot", json!(14)),
        ("/calls/instrumented", json!(1)),
    ];
    for (path, value) in edits {
        let mut x = v.clone();
        *x.pointer_mut(path).unwrap() = value;
        assert!(
            RuntimeSnapshot::parse(&serde_json::to_vec(&x).unwrap(), &b, &art).is_err(),
            "{path}"
        );
    }
}
#[test]
fn direct_buffer_partial_budgets_do_not_claim_a_tail_was_analyzed() {
    let (b, art, v) = buffer_fixture();
    let s = parse(&v, &b, &art);
    for budget in [1, 8, 9, 16] {
        let (mut p, mut f) = analyze(&b, budget);
        s.resolve(&mut p, &mut f).unwrap();
        assert!(f.is_incomplete());
        assert!(f.analysis_work <= budget);
    }
}
fn analyze(b: &[u8], budget: usize) -> (GhidraProgram, ghidra_hopper_rs::cfg_tags::Procedure) {
    let image = MachOImage::parse(b).unwrap();
    let mut p = GhidraProgram::from_macho_image("jni", &image);
    let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut p,
        "jni",
        0x1200,
        budget,
        &[],
    );
    (p, f)
}
#[test]
fn observed_jni_tail_returns_natively_and_preserves_unknown_exception_semantics() {
    let (b, art, v) = fixture();
    let s = parse(&v, &b, &art);
    let (mut p, mut f) = analyze(&b, 128);
    assert!(f.is_incomplete());
    assert_eq!(s.resolve(&mut p, &mut f).unwrap(), 2);
    assert!(!f.is_incomplete());
    assert_eq!(f.analysis_work, 23);
    let insns: Vec<_> = f
        .basic_blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .collect();
    let call = insns.iter().find(|i| i.address == 0x1224).unwrap();
    assert!(matches!(
        call.pcode[0],
        PcodeOp::RuntimeJniCall {
            slot: 6,
            tail: false,
            ..
        }
    ));
    assert!(matches!(call.pcode.last(), Some(PcodeOp::ClobberNzcv)));
    let tail = insns.iter().find(|i| i.address == 0x1248).unwrap();
    assert!(matches!(
        tail.pcode[0],
        PcodeOp::RuntimeJniCall {
            target: 0x70001240,
            slot: 14,
            tail: true,
            ..
        }
    ));
    assert!(!insns
        .iter()
        .any(|i| matches!(i.pcode.first(), Some(PcodeOp::NoReturnCall { .. }))));
    let text = f.render_four_modes(p.text_segment().unwrap());
    assert!(text.contains("tailcall_runtime_jni(\"ThrowNew\""));
    assert!(text.contains("supplied offline capture; stable JNIEnv table assumed"));
    assert!(text.contains("pending exception possible; native return allowed"));
    assert!(p
        .references
        .iter()
        .any(|r| r.from_addr == 0x1248 && r.to_addr == 0x70001240));
    assert!(s.resolve(&mut p, &mut f).is_err()); // Rebuild before changing runtime context.
}
#[test]
fn rejected_capture_cannot_silently_become_a_static_binding() {
    let (b, art, v) = fixture();
    for (pointer, value) in [
        ("/binary_sha256", json!("bad")),
        ("/runtime_sha256", json!("bad")),
        ("/abi", json!("x86_64")),
        ("/x0", json!("0x80000008")),
        ("/x3", json!("0x70001200")),
        ("/restored_word", json!("0xd503201f")),
        ("/table_stable", json!(false)),
        ("/calls/message_verified", json!(false)),
        ("/calls/trap_hits", json!(0)),
        ("/calls/uninstrumented", json!(-1)),
        (
            "/calls/exception_class",
            json!("java/lang/RuntimeException"),
        ),
        ("/target_module/base", json!("0x60001000")),
        ("/slots/1/name", json!("Throw")),
        ("/slots/1/index", json!(6)),
        ("/slots/1/code16", json!("00000000000000000000000000000000")),
        ("/slots/1/module/offset", json!("0x1244")),
        ("/slots/1/module/path", json!("/tmp/libfake.so")),
        (
            "/maps/0",
            json!("60001000-60002000 r--p 00000000 00:00 1 /data/fixture.so"),
        ),
        (
            "/maps/1",
            json!("70001000-70002000 rw-p 00000000 00:00 2 /apex/com.android.art/lib64/libart.so"),
        ),
        (
            "/maps/2",
            json!("70002000-70002008 r--p 00000000 00:00 2 /apex/com.android.art/lib64/libart.so"),
        ),
        (
            "/maps/3",
            json!("80000000-80001000 ---p 00000000 00:00 0 [anon:JNIEnv]"),
        ),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            RuntimeSnapshot::parse(&serde_json::to_vec(&bad).unwrap(), &b, &art).is_err(),
            "{pointer}"
        );
    }
    assert!(RuntimeSnapshot::parse(b"{}", &b, &art).is_err());
    assert!(RuntimeSnapshot::parse(&vec![b' '; 262145], &b, &art).is_err());
}
#[test]
fn requires_env_provenance_full_width_correct_slot_and_all_reaching_paths() {
    let (b, art, v) = fixture();
    for (index, word) in [
        (6, 0xaa0003e9),  // env saved in a volatile register; X20 has no provenance.
        (13, 0xaa0903e0), // volatile X9 was clobbered by FindClass.
        (16, 0xf9403503), // unknown table slot 13.
        (16, 0xb9403903), // narrow load cannot be a function pointer.
        (16, 0xf9403a63), // message pointer is not a JNIEnv table.
        (9, 0xd63f0120),  // opaque call cannot preserve table assumptions.
        (14, 0xf9000282), // overwrite JNIEnv->functions.
        (19, 0x17fffffd), // null branch rejoins with an unknown X8/X0.
    ] {
        let mut changed = b.clone();
        p32(&mut changed, 0x200 + index * 4, word);
        let mut profile = v.clone();
        profile["binary_sha256"] = json!(sha256(&changed));
        let s = parse(&profile, &changed, &art);
        let (mut p, mut f) = analyze(&changed, 128);
        s.resolve(&mut p, &mut f).unwrap();
        assert!(f.is_incomplete(), "{index}:{word:x}");
        assert!(
            f.basic_blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .all(|i| !matches!(
                    i.pcode.first(),
                    Some(PcodeOp::RuntimeJniCall { tail: true, .. })
                )),
            "{index}:{word:x}"
        );
    }
}
#[test]
fn budgets_live_code_changes_and_wrong_entry_never_inherit_a_capture() {
    let (b, art, v) = fixture();
    let s = parse(&v, &b, &art);
    for budget in [1, 9, 10, 18] {
        let (mut p, mut f) = analyze(&b, budget);
        s.resolve(&mut p, &mut f).unwrap();
        assert!(f.is_incomplete());
        assert!(f.analysis_work <= budget);
    }
    let (mut p, mut f) = analyze(&b, 128);
    p32(&mut p.segments[0].data, 0x240, 0xd503201f);
    assert!(s.resolve(&mut p, &mut f).is_err());
    assert!(f.is_incomplete());
    let (mut p, mut f) = analyze(&b, 128);
    f.entry_address += 4;
    assert!(s.resolve(&mut p, &mut f).is_err());
}
#[test]
fn binding_uses_dataflow_instead_of_a_fixed_saved_register_number() {
    let (mut b, art, mut v) = fixture();
    for (index, word) in [(6, 0xaa0003f5), (11, 0xf94002a8), (13, 0xaa1503e0)] {
        p32(&mut b, 0x200 + index * 4, word);
    }
    v["binary_sha256"] = json!(sha256(&b));
    let s = parse(&v, &b, &art);
    let (mut p, mut f) = analyze(&b, 128);
    assert_eq!(s.resolve(&mut p, &mut f).unwrap(), 2);
    assert!(!f.is_incomplete());
}
