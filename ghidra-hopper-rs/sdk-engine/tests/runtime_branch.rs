use ghidra_hopper_rs::{
    cfg_tags::Procedure,
    ghidra_port::{GhidraProgram, HopperHybridObjcAnalyzerRs},
    jni_runtime::sha256,
    macho_objc::MachOImage,
    pcode_sleigh::PcodeOp,
    runtime_branch::RuntimeBranchSnapshot,
};
use serde_json::{json, Value};
fn p32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn p64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn elf() -> Vec<u8> {
    let mut b = vec![0; 0x600];
    b[..6].copy_from_slice(b"\x7fELF\x02\x01");
    b[18..20].copy_from_slice(&183u16.to_le_bytes());
    p64(&mut b, 32, 64);
    p32(&mut b, 64, 1);
    p32(&mut b, 68, 5);
    b[54..56].copy_from_slice(&56u16.to_le_bytes());
    b[56..58].copy_from_slice(&1u16.to_le_bytes());
    p64(&mut b, 80, 0x1000);
    p64(&mut b, 96, 0x600);
    p64(&mut b, 104, 0x600);
    b
}
fn fixture(null: bool) -> (Vec<u8>, Vec<u8>, Value) {
    let mut b = elf();
    let words = if null {
        [0xa9bf7bf0, 0x90000010, 0xf9428211, 0x91140210, 0xd61f0220]
    } else {
        [0xd503245f, 0xb4000060, 0xaa0003f0, 0xd61f0200, 0xd65f03c0]
    };
    for (n, w) in words.iter().enumerate() {
        p32(&mut b, 0x200 + n * 4, *w);
    }
    let mut provider = elf();
    for (n, w) in [0xd503201f, 0xd503201f, 0xd503201f, 0xd65f03c0]
        .iter()
        .enumerate()
    {
        p32(&mut provider, 0x200 + n * 4, *w);
    }
    if null {
        provider = b.clone();
    }
    let mut regs = vec![json!("0x0"); 31];
    regs[0] = json!(if null { "0x0" } else { "0x70001200" });
    regs[16] = json!(if null { "0x60001500" } else { "0x70001200" });
    let code = provider[0x200..0x210]
        .iter()
        .map(|n| format!("{n:02x}"))
        .collect::<String>();
    let v = json!({"schema":1,"source":"android-process-private-branch","abi":"arm64-v8a","pid":123,"fingerprint":"synthetic-test",
        "binary_sha256":sha256(&b),"runtime_sha256":sha256(&provider),"entry":"0x60001200","branch_pc":if null{"0x60001210"}else{"0x6000120c"},
        "original_word":if null{"0xd61f0220"}else{"0xd61f0200"},"restored_word":if null{"0xd61f0220"}else{"0xd61f0200"},
        "target_module":{"base":"0x60000000","offset":"0x1200","path":"/data/fixture.so"},"entry_x0":if null{"0x0"}else{"0x70001200"},
        "target_register":if null{17}else{16},"observed_target":if null{"0x0"}else{"0x70001200"},"registers":regs,
        "kind":if null{"null-plt-resolver"}else{"registered-callback"},
        "target":{"module":{"base":"0x70000000","offset":"0x1200","path":"/data/provider.so"},"code16":code},
        "memory":{"offset":"0x1500","address":"0x60001500","before":"0x0","after":"0x0"},
        "calls":{"uninstrumented":1,"instrumented":1,"null_guard":true,"registered":true,"finalized":true,"trap_hits":1,
            "plain_faults":1,"traced_faults":1,"signal":11,"fault_pc":"0x0","fault_address":"0x0"},
        "maps":["60001000-60002000 r-xp 00000000 00:00 1 /data/fixture.so","70001000-70002000 r-xp 00000000 00:00 2 /data/provider.so"]});
    (b, provider, v)
}
fn parse(v: &Value, b: &[u8], r: &[u8]) -> RuntimeBranchSnapshot {
    RuntimeBranchSnapshot::parse(&serde_json::to_vec(v).unwrap(), b, r).unwrap()
}
fn analyze(b: &[u8], budget: usize) -> (GhidraProgram, Procedure) {
    let image = MachOImage::parse(b).unwrap();
    let mut p = GhidraProgram::from_macho_image("branch", &image);
    let f = HopperHybridObjcAnalyzerRs::new().analyze_function_zero_ipc(
        &mut p,
        "branch",
        0x1200,
        budget,
        &[],
    );
    (p, f)
}
#[test]
fn callback_is_conditional_external_tail_and_does_not_decode_provider() {
    let (b, r, v) = fixture(false);
    let s = parse(&v, &b, &r);
    let (mut p, mut f) = analyze(&b, 128);
    assert!(f.is_incomplete());
    assert_eq!(s.resolve(&mut p, &mut f).unwrap(), 1);
    assert!(!f.is_incomplete());
    assert_eq!(f.analysis_work, 5);
    assert!(f
        .basic_blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|i| matches!(
            i.pcode[0],
            PcodeOp::RuntimeTailCall {
                target: 0x70001200,
                ..
            }
        )));
    assert!(f.basic_blocks.iter().all(|b| b.start_address < 0x70000000));
    let t = f.render_four_modes(p.segment_at(0x1200).unwrap());
    assert!(t.contains("entry x0"));
    assert!(t.contains("signature unknown"));
    assert!(t.contains("native return allowed"));
    assert!(s.resolve(&mut p, &mut f).is_err());
}
#[test]
fn observed_null_resolver_is_a_fault_and_remains_incomplete() {
    let (b, r, v) = fixture(true);
    let s = parse(&v, &b, &r);
    let (mut p, mut f) = analyze(&b, 128);
    assert_eq!(s.resolve(&mut p, &mut f).unwrap(), 1);
    assert!(f.is_incomplete());
    assert_eq!(f.analysis_work, 5);
    let t = f.render_four_modes(p.segment_at(0x1200).unwrap());
    assert!(t.contains("runtime_branch_fault(target=0x0"));
    assert!(t.contains("not callable"));
    assert!(!t.contains("tailcall_runtime("));
}
#[test]
fn corrupted_images_maps_registers_and_execution_claims_are_rejected() {
    for null in [false, true] {
        let (b, r, v) = fixture(null);
        let mut edits = vec![
            ("/binary_sha256", json!("bad")),
            ("/runtime_sha256", json!("bad")),
            ("/pid", json!(0)),
            ("/fingerprint", json!("")),
            ("/original_word", json!("0x0")),
            ("/restored_word", json!("0x0")),
            ("/entry", json!("0x60001204")),
            ("/target_register", json!(31)),
            ("/calls/trap_hits", json!(0)),
            ("/observed_target", json!("0xdead")),
            (
                "/maps/0",
                json!("60001000-60002000 r--p 00000000 00:00 1 /data/fixture.so"),
            ),
        ];
        if null {
            edits.extend([
                ("/memory/after", json!("0x4")),
                ("/memory/address", json!("0x60001508")),
                ("/calls/fault_pc", json!("0x4")),
                ("/calls/signal", json!(7)),
            ]);
        } else {
            edits.extend([
                ("/entry_x0", json!("0x1")),
                ("/target/code16", json!("00")),
                ("/target/module/base", json!("0x71000000")),
                ("/calls/registered", json!(false)),
            ]);
        }
        for (path, value) in edits {
            let mut x = v.clone();
            *x.pointer_mut(path).unwrap() = value;
            assert!(
                RuntimeBranchSnapshot::parse(&serde_json::to_vec(&x).unwrap(), &b, &r).is_err(),
                "{null}: {path}"
            );
        }
    }
}
#[test]
fn full_width_input_and_exact_memory_provenance_are_required() {
    for (null, offset, word) in [
        (false, 8, 0x2a0003f0),
        (false, 0, 0xf9000000),
        (false, 8, 0xd2800010),
        (true, 8, 0xaa1f03f1),
        (true, 8, 0xf9428611),
        (true, 0, 0xf9000000),
    ] {
        let (mut b, mut r, mut v) = fixture(null);
        p32(&mut b, 0x200 + offset, word);
        v["binary_sha256"] = json!(sha256(&b));
        if null {
            r = b.clone();
            v["runtime_sha256"] = json!(sha256(&r));
        }
        let s = parse(&v, &b, &r);
        let (mut p, mut f) = analyze(&b, 128);
        assert_eq!(s.resolve(&mut p, &mut f).unwrap(), 0, "{word:x}");
        assert!(f.is_incomplete());
    }
}
#[test]
fn partial_budget_wrong_entry_and_changed_live_code_cannot_inherit_snapshot() {
    for null in [false, true] {
        let (b, r, v) = fixture(null);
        let s = parse(&v, &b, &r);
        for budget in 1..5 {
            let (mut p, mut f) = analyze(&b, budget);
            s.resolve(&mut p, &mut f).unwrap();
            assert!(f.is_incomplete());
            assert!(f.analysis_work <= budget);
        }
        let (mut p, mut f) = analyze(&b, 128);
        f.entry_address += 4;
        assert!(s.resolve(&mut p, &mut f).is_err());
        let (mut p, mut f) = analyze(&b, 128);
        p.segment_at_mut(0x1200).unwrap().data[0x200] = 0;
        assert!(s.resolve(&mut p, &mut f).is_err());
    }
}
