use ghidra_hopper_rs::ghidra_port::{
    GhidraJumpTable, GhidraProgram, HopperHybridObjcAnalyzerRs,
};
use ghidra_hopper_rs::macho_objc::MachOImage;
use ghidra_hopper_rs::mcp_server;
use ghidra_hopper_rs::pe_x64::Pe64Image;
use ghidra_hopper_rs::segment::{OperandFormat, OperandFormatKind, Segment, Tag};
use std::env;
use std::fs;
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("demo");

    if mode == "mcp" {
        let summary = r#"{"engine":"ghidra-hopper-rs","status":"ready","ported_analyzers":["ObjcMessageAnalyzer","DecompilerSwitchAnalysisCmd","HopperHybridObjcAnalyzer"]}"#;
        let _ = mcp_server::run_stdio_mcp_loop(summary);
        return;
    }

    if mode == "pe-slice" {
        let usage = "Usage: ghidra-hopper-rs pe-slice <file> [offset=0] [length=4096]";
        let Some(target_path) = args.get(2) else {
            eprintln!("{usage}");
            return;
        };
        let number_arg = |idx: usize, default: usize| -> Result<usize, String> {
            match args.get(idx) {
                None => Ok(default),
                Some(s) => s.parse().map_err(|e| format!("Invalid number `{s}`: {e}")),
            }
        };
        let (offset, length) = match (number_arg(3, 0), number_arg(4, 4096)) {
            (Ok(offset), Ok(length)) => (offset, length),
            (Err(e), _) | (_, Err(e)) => {
                eprintln!("{e}\n{usage}");
                return;
            }
        };
        match fs::read(target_path) {
            Ok(raw_file) => {
                let end = offset.saturating_add(length).min(raw_file.len());
                match raw_file.get(offset..end) {
                    Some(slice) if offset < raw_file.len() => {
                        println!(
                            "=== ghidra-hopper-rs Byte Slice: `{}` [offset={}..{}] ===",
                            target_path, offset, end
                        );
                        println!("{}", String::from_utf8_lossy(slice));
                    }
                    _ => eprintln!(
                        "Offset {} out of bounds (file size {})",
                        offset,
                        raw_file.len()
                    ),
                }
            }
            Err(e) => eprintln!("File read error: {}", e),
        }
        return;
    }

    if mode == "pe-scan" {
        let Some(target_path) = args.get(2) else {
            eprintln!("Usage: ghidra-hopper-rs pe-scan <pe-binary-path> [kw1,kw2,...]");
            return;
        };
        let kw_arg = args.get(3).map(String::as_str).unwrap_or("");
        let keywords: Vec<&str> = kw_arg.split(',').filter(|s| !s.is_empty()).collect();
        let t0 = Instant::now();
        match fs::read(target_path) {
            Ok(raw_file) => match Pe64Image::parse(&raw_file) {
                Ok(pe) => {
                    let ghidra_prog = GhidraProgram::from_pe_image(target_path, &pe);
                    println!("=== ghidra-hopper-rs PE64 Analysis: `{}` ===", target_path);
                    print_pe_summary(&pe, &ghidra_prog);
                    if !keywords.is_empty() {
                        let matches = Pe64Image::scan_matching_strings(&raw_file, &keywords, 80);
                        println!("    - Keyword Matches  : {:?}: {} hits", keywords, matches.len());
                        for m in matches {
                            println!("      * {}", m);
                        }
                    } else {
                        let shown = pe.notable_strings.len().min(20);
                        println!("    - Notable Strings  : {:?}", &pe.notable_strings[..shown]);
                    }
                    println!("    - Elapsed          : {:.3?}", t0.elapsed());
                }
                Err(e) => eprintln!("PE64 parse error: {}", e),
            },
            Err(e) => eprintln!("File read error: {}", e),
        }
        return;
    }

    let start_time = Instant::now();
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║  Ghidra 12.3 + Hopper Hybrid Engine (Rust Port) - Zero-IPC Pipeline    ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝");

    let default_hopper_bin = "/Applications/Hopper Disassembler.app/Contents/MacOS/Hopper Disassembler";
    let target_path = args.get(2).map(String::as_str).unwrap_or(default_hopper_bin);

    if let Ok(raw_file) = fs::read(target_path) {
        let t0 = Instant::now();
        match MachOImage::parse(&raw_file) {
            Ok(img) => {
                let ghidra_prog = GhidraProgram::from_macho_image("Hopper.app", &img);
                let elapsed = t0.elapsed();
                println!(
                    "\n[1] Zero-Copy Rust Mach-O -> GhidraProgram Loader: `{}`",
                    target_path
                );
                println!(
                    "    - Universal Binary : {} ({} slices, selected `{}`)",
                    img.is_universal,
                    img.slices.len(),
                    img.selected_arch
                );
                println!(
                    "    - Entry Point      : {:?}",
                    img.entry_point.map(|e| format!("0x{:x}", e))
                );
                println!("    - Segments Loaded  : {}", ghidra_prog.segments.len());
                println!(
                    "    - ObjC Classes     : {} {:?}",
                    img.objc.class_names.len(),
                    &img.objc.class_names[..img.objc.class_names.len().min(6)]
                );
                println!(
                    "    - ObjC Stubs Bound : {} stubs pre-indexed in SymbolTable (Zero-IPC)",
                    ghidra_prog.objc_stub_selectors.len()
                );
                println!("    - Load Latency     : {:.3?}", elapsed);
            }
            Err(macho_err) => match Pe64Image::parse(&raw_file) {
                Ok(pe) => {
                    let ghidra_prog = GhidraProgram::from_pe_image(target_path, &pe);
                    let elapsed = t0.elapsed();
                    println!("\n[1] Rust PE64 -> GhidraProgram Loader: `{}`", target_path);
                    println!("    - Segments Loaded  : {}", ghidra_prog.segments.len());
                    print_pe_summary(&pe, &ghidra_prog);
                    println!("    - Load Latency     : {:.3?}", elapsed);
                }
                Err(pe_err) => {
                    println!("    [!] Not Mach-O ({}) and not PE32+ ({})", macho_err, pe_err);
                }
            },
        }
    }

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
        first_bb.tags.insert(Tag::new("entry-objc-bridge", "#FF6B6B"));
    }

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

/// Prints what `Pe64Image::parse` and `GhidraProgram::from_pe_image` produced.
fn print_pe_summary(pe: &Pe64Image, prog: &GhidraProgram) {
    println!("    - Architecture     : {} (machine=0x{:04x})", pe.arch_name, pe.machine);
    println!("    - ImageBase        : 0x{:x}", pe.image_base);
    println!(
        "    - Entry Point      : RVA 0x{:x} (VA 0x{:x})",
        pe.entry_point_rva,
        pe.va(pe.entry_point_rva)
    );
    let sections: Vec<String> = pe
        .sections
        .iter()
        .map(|s| format!("{}(0x{:x})", s.name, s.virtual_size))
        .collect();
    println!("    - Sections         : {} {:?}", sections.len(), sections);
    println!(
        "    - .pdata Functions : {} {} entries; {} procedure heads marked as ByteType::Procedure",
        pe.runtime_functions.len(),
        pe.arch_name,
        pe.procedures_marked
    );
    let sample: Vec<&String> = pe.exports.values().take(12).collect();
    println!(
        "    - Exports          : {} named at {} addresses, {} forwarded (sample: {:?})",
        pe.named_export_count,
        pe.exports.len(),
        pe.forwarded_exports.len(),
        sample
    );
    let dlls = &pe.imported_dlls[..pe.imported_dlls.len().min(20)];
    println!(
        "    - Imports          : {} functions from {} DLLs {:?}",
        pe.imported_functions.len(),
        pe.imported_dlls.len(),
        dlls
    );
    println!(
        "    - Ghidra Program   : {} symbols, {} bookmarks",
        prog.symbol_table.len(),
        prog.bookmarks.len()
    );
    for note in &pe.notes {
        println!("    - Note             : {}", note);
    }
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
            Some("[HopperGhidraBridge initWithUTF8String:@\"Hopper+Ghidra Rust Engine\"]")
        );
        assert!(prog
            .references
            .iter()
            .any(|r| r.from_addr == 0x10000000c
                && r.ref_type == GhidraRefType::CallOverrideUnconditional));

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
}
