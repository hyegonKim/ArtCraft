---
name: ghidra-hopper-rs
description: >-
  Rust-native reverse engineering engine combining Ghidra 12.3 P-Code/SLEIGH
  decompiler with Hopper Disassembler's O(1) ByteType state machine, Zero-IPC
  Objective-C selector recovery, SwitchHintTable jump-table overrides, 4-Mode
  unified view (ASM, CFG, Pseudocode, Hex), and STDIO MCP server.
---

# ghidra-hopper-rs

Use `ghidra-hopper-rs` for high-speed Zero-IPC Mach-O / ARM64 / Objective-C & Swift analysis, 4-mode unified disassembly/CFG/pseudocode/hex inspection, and Ghidra 12.3 + Hopper hybrid analysis.

## Locations

- **Repository**: `c:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs`
- **Release Binary**: `c:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\target\release\ghidra-hopper-rs.exe`
- **Ghidra 12.3 Java Extension**: `c:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\ghidra-extension\`

## CLI Usage

1. **Run 4-Mode Hybrid Demo / Analyze Mach-O Binary**:
   ```powershell
   & "c:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\target\release\ghidra-hopper-rs.exe" demo "<path-to-macho-binary>"
   ```

2. **Run Built-in STDIO MCP Server**:
   ```powershell
   & "c:\Projects\AI_BibeCoding\ArtCraft\ghidra-hopper-rs\target\release\ghidra-hopper-rs.exe" mcp
   ```
   Exposes MCP tools:
   - `binary_overview`: Return Hopper Byte-State + Ghidra P-Code hybrid overview
   - `decompile_objc`: Decompile procedure with Ghidra P-Code and Hopper `CALL_OBJC` selectors
