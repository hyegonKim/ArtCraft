use ghidra_hopper_rs::ghidra_port::{GhidraJumpTable, GhidraProgram, HopperHybridObjcAnalyzerRs};
use ghidra_hopper_rs::macho_objc::MachOImage;
use ghidra_hopper_rs::mcp_server;
use ghidra_hopper_rs::segment::{demangle_symbol, OperandFormat, OperandFormatKind, Segment, Tag};
use std::env;
use std::fs;
use std::path::{Component, Path};
use std::time::Instant;

/// Builds a synthetic ARM64 `GhidraProgram` containing:
/// - ADRP + ADD loading an ASCII string (`"Hopper+Ghidra Rust Engine"`)
/// - MOVZ setting an argument
/// - `BL` to an Objective-C selector stub (`objc_msgSend$initWithUTF8String:`)
/// - `CBZ` conditional branch
/// - `BR` indirect jump resolved via `GhidraJumpTable` (`DecompilerSwitchAnalysisCmd` port)
/// - `RET`
fn build_demo_ghidra_program() -> (GhidraProgram, Vec<GhidraJumpTable>) {
    let base: u64 = 0x100000000;
    let mut bytes = vec![0u8; 0x200];

    let msg = b"Hopper+Ghidra Rust Engine\0";
    bytes[0x100..0x100 + msg.len()].copy_from_slice(msg);

    let words: [u32; 7] = [
        0x90000002, // 0x100000000: adrp x2, 0x100000000
        0x91040042, // 0x100000004: add  x2, x2, #0x100 ("Hopper+Ghidra Rust Engine")
        0x52800541, // 0x100000008: mov  w1, #0x2a (42)
        0x94000020, // 0x10000000c: bl   0x10000008c (objc_msgSend$initWithUTF8String:)
        0xb4000060, // 0x100000010: cbz  x0, 0x10000001c
        0xd61f0060, // 0x100000014: br   x3 (Switch jump table!)
        0xd65f03c0, // 0x100000018: ret
    ];
    for (i, w) in words.iter().enumerate() {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&w.to_le_bytes());
    }
    let epilogue: [u32; 2] = [
        0x52800000, // 0x10000001c: mov w0, #0x0
        0xd65f03c0, // 0x100000020: ret
    ];
    for (i, w) in epilogue.iter().enumerate() {
        let off = 0x1c + i * 4;
        bytes[off..off + 4].copy_from_slice(&w.to_le_bytes());
    }

    let mut seg = Segment::new("__TEXT", base, 0, bytes);
    seg.scan_ascii_strings(4);

    seg.operand_formats.insert(
        (base + 8, 1),
        OperandFormat {
            base: OperandFormatKind::Decimal,
            ..Default::default()
        },
    );

    let mut prog = GhidraProgram::new("HopperHybridDemo", seg);
    prog.objc_stub_selectors
        .insert(base + 0x8c, "initWithUTF8String:".to_string());

    let jump_table = GhidraJumpTable::new(base + 0x14, vec![base + 0x18, base + 0x1c]);
    (prog, vec![jump_table])
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("Error: {message}");
    std::process::exit(1)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("demo");
    if mode == "ghidra-decompile" {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            match ghidra_hopper_rs::ghidra_backend::Config::parse(&args[2..])
                .and_then(|config| ghidra_hopper_rs::ghidra_backend::decompile(&config))
            {
                Ok(result) => println!("{result}"),
                Err(error) => {
                    println!("{}", error.json());
                    std::process::exit(1);
                }
            }
            return;
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            println!(
                "{}",
                serde_json::json!({"schema":1,"status":"error","provider":"external_ghidra","code":"unsupported_host","message":"external Ghidra currently requires macOS arm64"})
            );
            std::process::exit(1);
        }
    }
    if !matches!(mode, "demo" | "inspect" | "decompile" | "mcp") {
        fail("Usage: ghidra-hopper-rs <inspect|decompile|demo|mcp|ghidra-decompile>");
    }
    if mode == "inspect" || (mode == "demo" && args.get(2).is_some()) {
        let path = args
            .get(2)
            .unwrap_or_else(|| fail("inspect requires a binary path"));
        let bytes = fs::read(path).unwrap_or_else(|err| fail(err));
        let image = MachOImage::parse(&bytes).unwrap_or_else(|err| fail(err));
        println!(
            "{}",
            serde_json::json!({"path":path,"architecture":image.selected_arch,"entry_point":image.entry_point,"segments":image.segments.len(),"objc_stubs_verified":image.objc.selector_stubs.len(),"status":"metadata_only","decompilation_performed":false})
        );
        return;
    }

    if mode == "mcp" {
        let summary = r#"{"engine":"ghidra-hopper-rs","version":"0.3.0","status":"no_document_loaded","provider_health":"not_checked","engine_scope":"partial_arm64","full_ssa_decompiler":false,"document_mutation":false}"#;
        if let Err(err) = mcp_server::run_stdio_mcp_loop(summary) {
            eprintln!("MCP transport error: {err}");
            std::process::exit(1);
        }
        return;
    }

    if mode == "decompile" {
        let target_path = args
            .get(2)
            .unwrap_or_else(|| fail("decompile requires a binary path"));
        let func_name = args.get(3).map(String::as_str).unwrap_or("entry");
        let addr_str = args
            .get(4)
            .map(String::as_str)
            .unwrap_or("0x100000000")
            .trim_start_matches("0x");
        let requested_entry = args.get(4).map(|_| {
            u64::from_str_radix(addr_str, 16)
                .unwrap_or_else(|_| fail("Invalid hexadecimal function address"))
        });
        let max_insns: usize = args
            .get(5)
            .map(|s| {
                s.parse()
                    .unwrap_or_else(|_| fail("Invalid instruction budget"))
            })
            .unwrap_or(16);
        if max_insns == 0 {
            fail("Instruction budget must be positive");
        }
        let (runtime_path, branch_context) = match args.get(6..) {
            None | Some([]) => (None, false),
            Some([flag, path]) if flag == "--jni-runtime" => (Some(path), false),
            Some([flag, path]) if flag == "--branch-runtime" => (Some(path), true),
            _ => fail(
                "Usage: decompile <binary> <name> <entry> <budget> [--jni-runtime <capture.json> | --branch-runtime <capture.json>]",
            ),
        };

        let start_time = Instant::now();
        let raw_file = fs::read(target_path).unwrap_or_else(|err| fail(err));
        let img = MachOImage::parse(&raw_file).unwrap_or_else(|err| fail(err));
        let entry_addr = requested_entry
            .or(img.entry_point)
            .unwrap_or_else(|| fail("Binary has no entry point; provide an explicit address"));
        if img.selected_arch != "arm64" {
            eprintln!(
                "Decompilation supports ARM64 only; selected architecture is {}",
                img.selected_arch
            );
            std::process::exit(1);
        }
        let mut prog = GhidraProgram::from_macho_image(target_path, &img);
        if prog
            .segment_at(entry_addr)
            .and_then(|seg| seg.read_bytes(entry_addr, 4))
            .is_none()
            || !entry_addr.is_multiple_of(4)
        {
            eprintln!(
                "Function entry 0x{:x} is unaligned or outside loaded segments",
                entry_addr
            );
            std::process::exit(1);
        }
        if let Some(seg) = prog.segment_at_mut(entry_addr) {
            seg.scan_ascii_strings(4);
        }

        let mut analyzer = HopperHybridObjcAnalyzerRs::new();
        let mut proc =
            analyzer.analyze_function_zero_ipc(&mut prog, func_name, entry_addr, max_insns, &[]);
        if let Some(path) = runtime_path {
            if fs::metadata(path).unwrap_or_else(|e| fail(e)).len() > 262144 {
                fail("JNI evidence size limit");
            }
            let profile = fs::read(path).unwrap_or_else(|e| fail(e));
            let json: serde_json::Value =
                serde_json::from_slice(&profile).unwrap_or_else(|e| fail(e));
            let relative = json["runtime_image"]
                .as_str()
                .unwrap_or_else(|| fail("JNI capture requires runtime_image"));
            if !Path::new(relative)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            {
                fail("runtime_image must be a relative path inside the capture directory");
            }
            let art_path = Path::new(path)
                .parent()
                .unwrap_or(Path::new("."))
                .join(relative);
            if fs::metadata(&art_path).unwrap_or_else(|e| fail(e)).len() > 64 * 1024 * 1024 {
                fail("ART image size limit");
            }
            let art = fs::read(art_path).unwrap_or_else(|e| fail(e));
            if branch_context {
                let snapshot = ghidra_hopper_rs::runtime_branch::RuntimeBranchSnapshot::parse(
                    &profile, &raw_file, &art,
                )
                .unwrap_or_else(|e| fail(e));
                let count = snapshot
                    .resolve(&mut prog, &mut proc)
                    .unwrap_or_else(|e| fail(e));
                print!("{}", snapshot.summary());
                println!("Runtime branch observations applied: {count}");
            } else {
                let snapshot = ghidra_hopper_rs::jni_runtime::RuntimeSnapshot::parse(
                    &profile, &raw_file, &art,
                )
                .unwrap_or_else(|e| fail(e));
                let count = snapshot
                    .resolve(&mut prog, &mut proc)
                    .unwrap_or_else(|e| fail(e));
                print!("{}", snapshot.summary());
                println!("JNI runtime bindings applied: {count}");
            }
        }
        if let Some(first_bb) = proc.basic_blocks.first_mut() {
            first_bb
                .tags
                .insert(Tag::new("real-binary-target", "#FF6B6B"));
        }

        println!("╔════════════════════════════════════════════════════════════════════════╗");
        println!("║  ghidra-hopper-rs Partial ARM64 Analysis (v0.3.0)                     ║");
        println!("╚════════════════════════════════════════════════════════════════════════╝");
        println!("Target Binary   : {}", target_path);
        println!(
            "Architecture    : {} (Universal={}, Slices={})",
            img.selected_arch,
            img.is_universal,
            img.slices.len()
        );
        println!(
            "Entry Point     : {:?}",
            img.entry_point.map(|e| format!("0x{:x}", e))
        );
        println!("Segments        : {}", prog.segments.len());
        println!("ObjC Classes    : {:?}", img.objc.class_names);
        println!(
            "ObjC Selectors  : {} indexed in SymbolTable",
            prog.objc_stub_selectors.len()
        );
        println!(
            "Procedure       : {} @ 0x{:x} ({} decoded; budget {})",
            func_name,
            entry_addr,
            proc.basic_blocks
                .iter()
                .map(|bb| bb.instructions.len())
                .sum::<usize>(),
            max_insns
        );
        println!("Discovered ObjC : {:?}", analyzer.discovered_messages);
        println!("Ghidra Refs     : {}", prog.references.len());
        for r in &prog.references {
            println!(
                "  * 0x{:x} -> 0x{:x} [{:?}] (source={:?})",
                r.from_addr, r.to_addr, r.ref_type, r.source
            );
        }
        println!("Ghidra Bookmarks: {}", prog.bookmarks.len());
        for bm in &prog.bookmarks {
            println!(
                "  * @ 0x{:x} [{} / {}]: {}",
                bm.address, bm.bookmark_type, bm.category, bm.comment
            );
        }
        if let Some(seg) = prog.segment_at(entry_addr) {
            println!("\n{}", proc.render_four_modes(seg));
        }
        println!("Elapsed         : {:.3?}", start_time.elapsed());
        if proc.is_incomplete() {
            std::process::exit(2);
        }
        return;
    }

    let start_time = Instant::now();
    println!("SYNTHETIC DEMO ONLY: no real binary is analyzed below");
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║  Ghidra 12.3_DEV + Hopper 6.6.0 Hybrid Engine (ghidra-hopper-rs v0.3.0)║");
    println!("║  Synced: Ghidra 4718f50d99 | Hopper 42.6316032 | REA 6.8.0 (15 Agents) ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝");

    // 2. Execute the Rust-ported Ghidra `HopperHybridObjcAnalyzerRs` + `GhidraJumpTable`
    println!(
        "\n[2] Running Ported `HopperHybridObjcAnalyzerRs` (ObjcMessageAnalyzer + DecompilerSwitchAnalysisCmd)..."
    );
    let (mut prog, jump_tables) = build_demo_ghidra_program();
    let mut analyzer = HopperHybridObjcAnalyzerRs::new();
    let mut proc = analyzer.analyze_function_zero_ipc(
        &mut prog,
        "-[HopperGhidraBridge bootstrapEngine:]",
        0x100000000,
        9,
        &jump_tables,
    );

    if let Some(first_bb) = proc.basic_blocks.first_mut() {
        first_bb
            .tags
            .insert(Tag::new("entry-objc-bridge", "#FF6B6B"));
    }
    // Hopper `renameRegister` + Objective-C ABI (`x0 -> self`, `x1 -> _cmd`, `x3 -> dispatchIndex`)
    proc.rename_register(3, "dispatchIndex");

    println!(
        "    - Swift 5 Demangler Demo   : `_$s6Hopper16HybridDecompilerC7analyzeSbSS_tF` -> `{}`",
        demangle_symbol("_$s6Hopper16HybridDecompilerC7analyzeSbSS_tF")
    );
    println!(
        "    - Hopper Register Aliases  : {:?}",
        proc.register_renames
    );

    println!(
        "    - Discovered ObjC Messages : {:?}",
        analyzer.discovered_messages
    );
    println!("    - Ghidra References Added  :");
    for r in &prog.references {
        println!(
            "      * 0x{:x} -> 0x{:x} [{:?}] (source={:?})",
            r.from_addr, r.to_addr, r.ref_type, r.source
        );
    }
    println!("    - Ghidra EOL Comments      : {:?}", prog.eol_comments);
    println!("    - Ghidra HopperTag Bookmarks:");
    for bm in &prog.bookmarks {
        println!(
            "      * @ 0x{:x} [{} / {}]: {}",
            bm.address, bm.bookmark_type, bm.category, bm.comment
        );
    }

    if let Some(seg) = prog.text_segment() {
        println!("\n{}", proc.render_four_modes(seg));
    }

    println!(
        "Total execution time: {:.3?} (Zero GC pauses, Zero Decompiler IPC pipe overhead)",
        start_time.elapsed()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghidra_hopper_rs::ghidra_port::GhidraRefType;
    use ghidra_hopper_rs::segment::ByteType;

    #[test]
    fn test_ported_ghidra_objc_and_switch_analyzer() {
        let (mut prog, jump_tables) = build_demo_ghidra_program();
        let mut analyzer = HopperHybridObjcAnalyzerRs::new();
        let proc = analyzer.analyze_function_zero_ipc(
            &mut prog,
            "test_proc",
            0x100000000,
            9,
            &jump_tables,
        );

        let states = HopperHybridObjcAnalyzerRs::verify_byte_state_slice(&prog, 0x100000000, 8);
        assert_eq!(states[0], ByteType::Procedure);
        assert_eq!(states[1], ByteType::Next);
        assert_eq!(states[4], ByteType::Code);

        // Verify ObjcMessageAnalyzer port created CALL_OVERRIDE_UNCONDITIONAL reference & EOL comment
        assert_eq!(analyzer.discovered_messages.len(), 1);
        assert_eq!(
            prog.eol_comments.get(&0x10000000c).map(String::as_str),
            Some("[x0 initWithUTF8String:\"Hopper+Ghidra Rust Engine\"]")
        );
        assert!(prog
            .references
            .iter()
            .any(|r| r.from_addr == 0x10000000c && r.ref_type == GhidraRefType::UnconditionalCall));

        // Verify DecompilerSwitchAnalysisCmd JumpTable port created COMPUTED_JUMP references
        let computed_jumps: Vec<_> = prog
            .references
            .iter()
            .filter(|r| r.from_addr == 0x100000014 && r.ref_type == GhidraRefType::ComputedJump)
            .collect();
        assert_eq!(computed_jumps.len(), 2);

        let seg = prog.text_segment().unwrap();
        let rendered = proc.render_four_modes(seg);
        assert!(rendered.contains("objc_msgSend$initWithUTF8String:"));
        assert!(rendered.contains("Hopper SwitchHint recovered 2 cases"));
        assert!(proc.basic_blocks.len() >= 2);
    }

    #[test]
    fn test_hopper_interactive_hotkeys_and_swift_demangler() {
        let (mut prog, jump_tables) = build_demo_ghidra_program();
        let mut analyzer = HopperHybridObjcAnalyzerRs::new();
        let mut proc = analyzer.analyze_function_zero_ipc(
            &mut prog,
            "-[HopperGhidraBridge bootstrapEngine:]",
            0x100000000,
            9,
            &jump_tables,
        );
        proc.rename_register(3, "dispatchIndex");

        // Verify Objective-C ABI auto-aliasing (`x0 -> self`, `x1 -> _cmd`) + custom `x3 -> dispatchIndex`
        assert_eq!(
            proc.register_renames.get(&0).map(String::as_str),
            Some("self")
        );
        assert_eq!(
            proc.register_renames.get(&1).map(String::as_str),
            Some("_cmd")
        );
        assert_eq!(
            proc.register_renames.get(&3).map(String::as_str),
            Some("dispatchIndex")
        );

        let seg = prog.text_segment_mut().unwrap();
        let rendered = proc.render_four_modes(seg);
        assert!(
            rendered.contains("self = [self initWithUTF8String:\"Hopper+Ghidra Rust Engine\"];")
        );
        assert!(rendered.contains("switch (dispatchIndex)"));

        // Verify Hopper Interactive Hotkeys ('U' Undefine on trailing byte -> clears entire 4-byte instruction, then 'P' Procedure)
        assert!(seg.apply_hopper_hotkey(0x100000002, 'U'));
        assert_eq!(seg.get_type_at_address(0x100000000), ByteType::Undefined);
        assert_eq!(seg.get_type_at_address(0x100000003), ByteType::Undefined);
        assert!(seg.apply_hopper_hotkey(0x100000000, 'P'));
        assert_eq!(seg.get_type_at_address(0x100000000), ByteType::Procedure);
        assert_eq!(seg.get_type_at_address(0x100000001), ByteType::Next);

        // Verify Swift 5 length-prefixed symbol demangler
        assert_eq!(
            demangle_symbol("_$s6Hopper16HybridDecompilerC7analyzeSbSS_tF"),
            "swift::Hopper.HybridDecompiler.analyze"
        );
    }
}
