---
name: ghidra-hopper-rs
description: >-
  Rust-native reverse engineering engine (v0.3.1) combining Ghidra 12.3_DEV / 12.2
  P-Code/SLEIGH decompiler and REA 6.10.1 bridge parity with Hopper Disassembler's
  O(1) ByteType state machine, Zero-IPC Objective-C selector recovery, SwitchHintTable
  jump-table overrides, 4-Mode unified view (ASM, CFG, Pseudocode, Hex), Windows/macOS
  headless Ghidra provider, and STDIO MCP server.
---

# ghidra-hopper-rs

Use `ghidra-hopper-rs` for high-speed Zero-IPC Mach-O / PE64 / ARM64 / Objective-C & Swift analysis, 4-mode unified disassembly/CFG/pseudocode/hex inspection, Ghidra 12.2/12.3 + Hopper hybrid analysis, and REA 6.10.1 native bridge parity.

## Locations

- **Repository**: `E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs`
- **Main Release Binary (PE64 + Hybrid + 9-tool MCP)**: `E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\target\release\ghidra-hopper-rs.exe`
- **SDK Engine Release Binary (ARM64 Verifier + Headless Ghidra Provider)**: `E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\sdk-engine\target\release\ghidra-hopper-rs.exe`
- **Ghidra 12.3 Java Extension**: `E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\sdk-engine\ghidra-extension\`

## CLI Usage

1. **Run 4-Mode Hybrid Demo / Analyze Mach-O or PE64 Binary**:
   ```powershell
   & "E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\target\release\ghidra-hopper-rs.exe" demo "<path-to-binary>"
   ```

2. **Run Headless External Ghidra 12.2/12.3 Decompiler (`sdk-engine`, Windows x86_64 & macOS arm64)**:
   ```powershell
   & "E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\sdk-engine\target\release\ghidra-hopper-rs.exe" ghidra-decompile "<arm64-macho>" 0x100001000 --ghidra-home "E:\Projects\AI_BibeCoding\ArtCraft\.toolchain\ghidra\ghidra_12.2_PUBLIC" --java-home "E:\Projects\AI_BibeCoding\ArtCraft\.toolchain\jdk\jdk-25"
   ```

3. **Run Built-in STDIO MCP Server**:
   ```powershell
   & "E:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\target\release\ghidra-hopper-rs.exe" mcp
   ```
   Exposes MCP tools:
   - `binary_overview`: Return Hopper Byte-State + Ghidra 12.3 P-Code + REA 6.10.1 hybrid overview
   - `decompile_objc`: Decompile procedure with Ghidra P-Code and Hopper `CALL_OBJC` selectors
   - `inspect_native_instruction`: Inspect Hopper `ByteType` + Ghidra instruction context
   - `search_procedures`: Search primary symbols and `objc_stub::<selector>` aliases
   - `apply_address_names`: Batch-validate and rename symbols (`UTF-16 <= 1024`)
   - `rea_caller_scan_page`: Bounded `512 KiB` caller-scan pagination
   - `inspect_native_load_image`: REA 6.10.1 segment & symbol inventory
   - `resolve_native_call_targets`: REA 6.10.1 direct/override/switch call target resolution
   - `annotate_native_function`: REA 6.10.1 native function label & comment annotation
