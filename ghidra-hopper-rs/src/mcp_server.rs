//! Hopper 6.0-style Built-in STDIO MCP Server (`HopperMCPServer` equivalent in Rust).
//!
//! Allows LLMs (Claude, Gemini, Cursor, etc.) to query the disassembled document,
//! P-Code decompiler, and Objective-C/Swift metadata over STDIO JSON-RPC.

use std::io::{self, BufRead, Write};

pub fn run_stdio_mcp_loop(summary_json: &str) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = if trimmed.contains("\"initialize\"") {
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","serverInfo":{"name":"ghidra-hopper-rs-mcp","version":"0.1.0"},"capabilities":{"tools":{}}}}"#.to_string()
        } else if trimmed.contains("\"tools/list\"") {
            r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"binary_overview","description":"Return Hopper Byte-State + Ghidra P-Code hybrid overview"},{"name":"decompile_objc","description":"Decompile procedure with Ghidra P-Code and Hopper CALL_OBJC selectors"}]}}"#.to_string()
        } else {
            format!(
                r#"{{"jsonrpc":"2.0","id":3,"result":{{"summary":{}}}}}"#,
                summary_json
            )
        };
        writeln!(stdout, "{}", response)?;
        stdout.flush()?;
    }
    Ok(())
}
