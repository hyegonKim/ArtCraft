//! Hopper 6.6 + REA 6.10.1 + Ghidra 12.3_DEV Built-in STDIO MCP Server.
//!
//! Zero-dependency JSON-RPC 2.0 STDIO server preserving request `id` and dispatching:
//! - `binary_overview`
//! - `decompile_objc`
//! - `inspect_native_instruction`
//! - `search_procedures`
//! - `apply_address_names`
//! - `rea_caller_scan_page`
//! - `inspect_native_load_image` (REA 6.10.1)
//! - `resolve_native_call_targets` (REA 6.10.1)
//! - `annotate_native_function` (REA 6.10.1)

use crate::ghidra_port::{GhidraJumpTable, GhidraProgram, HopperHybridObjcAnalyzerRs};
use crate::segment::Segment;
use std::io::{self, BufRead, Write};

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn extract_raw_id(line: &str) -> String {
    if let Some(pos) = line.find("\"id\"") {
        let rest = line[pos + 4..].trim_start();
        if let Some(after_colon) = rest.strip_prefix(':') {
            let v = after_colon.trim_start();
            if let Some(stripped) = v.strip_prefix('"') {
                let mut escaped = false;
                for (idx, ch) in stripped.char_indices() {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        return format!("\"{}\"", &stripped[..idx]);
                    }
                }
            } else {
                let end = v
                    .find(|c: char| matches!(c, ',' | '}' | ' ' | '\t' | '\r' | '\n'))
                    .unwrap_or(v.len());
                if end > 0 {
                    return v[..end].to_string();
                }
            }
        }
    }
    "1".to_string()
}

fn extract_str_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut search_from = 0;
    while let Some(rel) = line[search_from..].find(&needle) {
        let pos = search_from + rel + needle.len();
        let rest = line[pos..].trim_start();
        if let Some(after_colon) = rest.strip_prefix(':') {
            let v = after_colon.trim_start();
            if let Some(stripped) = v.strip_prefix('"') {
                let mut out = String::new();
                let mut escaped = false;
                for ch in stripped.chars() {
                    if escaped {
                        out.push(ch);
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        return Some(out);
                    } else {
                        out.push(ch);
                    }
                }
            }
        }
        search_from = pos;
    }
    None
}

fn extract_u64_field(line: &str, key: &str) -> Option<u64> {
    if let Some(s) = extract_str_field(line, key) {
        let clean = s.trim().trim_start_matches("0x").trim_start_matches("0X");
        return u64::from_str_radix(clean, 16)
            .ok()
            .or_else(|| s.trim().parse().ok());
    }
    let needle = format!("\"{key}\"");
    let pos = line.find(&needle)? + needle.len();
    let rest = line[pos..].trim_start().strip_prefix(':')?.trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    rest[..end].parse().ok()
}

fn build_active_program() -> (GhidraProgram, String) {
    let base: u64 = 0x100000000;
    let mut bytes = vec![0u8; 0x200];
    let msg = b"Hopper+Ghidra Rust Engine\0";
    bytes[0x100..0x100 + msg.len()].copy_from_slice(msg);
    let words: [u32; 9] = [
        0x90000002, 0x91040042, 0x52800541, 0x94000020, 0xb4000060, 0xd61f0060, 0xd65f03c0,
        0x52800000, 0xd65f03c0,
    ];
    for (i, w) in words.iter().enumerate() {
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&w.to_le_bytes());
    }
    let mut seg = Segment::new("__TEXT", base, 0, bytes);
    seg.scan_ascii_strings(4);
    let mut prog = GhidraProgram::new("HopperHybridDemo", seg);
    prog.objc_stub_selectors
        .insert(base + 0x8c, "initWithUTF8String:".to_string());
    prog.symbol_table
        .insert(base + 0x8c, "objc_msgSend$initWithUTF8String:".to_string());
    let jt = GhidraJumpTable::new(base + 0x14, vec![base + 0x18, base + 0x1c]);
    let mut analyzer = HopperHybridObjcAnalyzerRs::new();
    let proc = analyzer.analyze_function_zero_ipc(
        &mut prog,
        "-[HopperGhidraBridge bootstrapEngine:]",
        base,
        9,
        &[jt],
    );
    let pseudo = prog
        .text_segment()
        .map(|s| proc.render_four_modes(s))
        .unwrap_or_default();
    (prog, pseudo)
}

fn wrap_tool_text(id_raw: &str, payload_json: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id_raw},"result":{{"content":[{{"type":"text","text":{}}}],"isError":false,"summary":{payload_json}}}}}"#,
        escape_json(payload_json)
    )
}

pub fn handle_mcp_line(
    line: &str,
    summary_json: &str,
    prog: &mut GhidraProgram,
    pseudo: &str,
) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.contains("\"notifications/") {
        return None;
    }
    let id_raw = extract_raw_id(trimmed);
    let method = extract_str_field(trimmed, "method").unwrap_or_default();

    match method.as_str() {
        "initialize" => Some(format!(
            r#"{{"jsonrpc":"2.0","id":{id_raw},"result":{{"protocolVersion":"2024-11-05","serverInfo":{{"name":"ghidra-hopper-rs-mcp","version":"{}"}},"capabilities":{{"tools":{{}}}}}}}}"#,
            env!("CARGO_PKG_VERSION")
        )),
        "ping" => Some(format!(
            r#"{{"jsonrpc":"2.0","id":{id_raw},"result":{{}}}}"#
        )),
        "tools/list" => Some(format!(
            r#"{{"jsonrpc":"2.0","id":{id_raw},"result":{{"tools":[{{"name":"binary_overview","description":"Return Hopper Byte-State + Ghidra 12.3 P-Code + REA 6.10.1 hybrid overview","inputSchema":{{"type":"object"}}}},{{"name":"decompile_objc","description":"Decompile procedure with Ghidra P-Code and Hopper CALL_OBJC selectors","inputSchema":{{"type":"object"}}}},{{"name":"inspect_native_instruction","description":"Inspect Hopper ByteType + Ghidra instruction context at address","inputSchema":{{"type":"object","properties":{{"address":{{"type":"string"}}}}}}}},{{"name":"search_procedures","description":"Search primary symbols and objc_stub::<selector> aliases","inputSchema":{{"type":"object","properties":{{"query":{{"type":"string"}}}}}}}},{{"name":"apply_address_names","description":"Apply validated symbol rename (UTF-16 <= 1024)","inputSchema":{{"type":"object","properties":{{"address":{{"type":"string"}},"name":{{"type":"string"}}}}}}}},{{"name":"rea_caller_scan_page","description":"Return bounded 512 KiB caller scan page for target address","inputSchema":{{"type":"object","properties":{{"address":{{"type":"string"}}}}}}}},{{"name":"inspect_native_load_image","description":"REA 6.10.1 load image segment & symbol inventory","inputSchema":{{"type":"object"}}}},{{"name":"resolve_native_call_targets","description":"REA 6.10.1 direct/override/switch call target resolution","inputSchema":{{"type":"object","properties":{{"address":{{"type":"string"}}}}}}}},{{"name":"annotate_native_function","description":"REA 6.10.1 native function label & comment annotation","inputSchema":{{"type":"object","properties":{{"address":{{"type":"string"}},"name":{{"type":"string"}},"comment":{{"type":"string"}}}}}}}}]}}}}"#
        )),
        "tools/call" => {
            let tool = extract_str_field(trimmed, "name").unwrap_or_default();
            let addr = extract_u64_field(trimmed, "address").unwrap_or(0x100000000);
            let payload = match tool.as_str() {
                "binary_overview" => summary_json.to_string(),
                "decompile_objc" => format!(
                    r#"{{"procedure":"-[HopperGhidraBridge bootstrapEngine:]","entry":"0x100000000","pseudocode":{}}}"#,
                    escape_json(pseudo)
                ),
                "inspect_native_instruction" => match prog.inspect_native_instruction(addr) {
                    Some(ctx) => format!(
                        r#"{{"address":"0x{:x}","instruction_start":{},"symbol":{},"objc_selector":{},"eol_comment":{},"outgoing_refs_count":{}}}"#,
                        ctx.address,
                        ctx.instruction_start
                            .map(|a| format!("\"0x{:x}\"", a))
                            .unwrap_or_else(|| "null".into()),
                        ctx.symbol
                            .as_deref()
                            .map(escape_json)
                            .unwrap_or_else(|| "null".into()),
                        ctx.objc_selector
                            .as_deref()
                            .map(escape_json)
                            .unwrap_or_else(|| "null".into()),
                        ctx.eol_comment
                            .as_deref()
                            .map(escape_json)
                            .unwrap_or_else(|| "null".into()),
                        ctx.outgoing_refs.len()
                    ),
                    None => format!(r#"{{"address":"0x{:x}","found":false}}"#, addr),
                },
                "search_procedures" => {
                    let q = extract_str_field(trimmed, "query").unwrap_or_default();
                    let hits = prog.search_procedures(&q, 32);
                    let items: Vec<String> = hits
                        .iter()
                        .map(|(a, n)| {
                            format!(r#"{{"address":"0x{:x}","name":{}}}"#, a, escape_json(n))
                        })
                        .collect();
                    format!(
                        r#"{{"query":{},"matches":[{}]}}"#,
                        escape_json(&q),
                        items.join(",")
                    )
                }
                "apply_address_names" => {
                    let sym_name = extract_str_field(trimmed, "symbol")
                        .or_else(|| extract_str_field(trimmed, "label"))
                        .unwrap_or_else(|| "renamed_proc".into());
                    let rep = prog.apply_address_names(&[(addr, sym_name)]);
                    format!(
                        r#"{{"applied":{},"rejected":{}}}"#,
                        rep.applied.len(),
                        rep.rejected.len()
                    )
                }
                "rea_caller_scan_page" => {
                    let page = prog.caller_scan_page(addr, 0);
                    let callers: Vec<String> = page
                        .callers
                        .iter()
                        .map(|c| format!("\"0x{:x}\"", c))
                        .collect();
                    format!(
                        r#"{{"target":"0x{:x}","callers":[{}],"budget_bytes":{}}}"#,
                        page.target_address,
                        callers.join(","),
                        page.budget_bytes
                    )
                }
                "inspect_native_load_image" => {
                    let info = prog.inspect_native_load_image();
                    format!(
                        r#"{{"program":{},"segments":{},"symbols":{},"objc_stubs":{},"references":{}}}"#,
                        escape_json(&info.program_name),
                        info.segments.len(),
                        info.symbol_count,
                        info.objc_stub_count,
                        info.reference_count
                    )
                }
                "resolve_native_call_targets" => {
                    let targets = prog.resolve_native_call_targets(addr);
                    let items: Vec<String> = targets
                        .iter()
                        .map(|t| {
                            format!(
                                r#"{{"to":"0x{:x}","ref_type":"{:?}","symbol":{}}}"#,
                                t.to_addr,
                                t.ref_type,
                                t.symbol
                                    .as_deref()
                                    .map(escape_json)
                                    .unwrap_or_else(|| "null".into())
                            )
                        })
                        .collect();
                    format!(
                        r#"{{"call_address":"0x{:x}","targets":[{}]}}"#,
                        addr,
                        items.join(",")
                    )
                }
                "annotate_native_function" => {
                    let comment = extract_str_field(trimmed, "comment");
                    let label = extract_str_field(trimmed, "label");
                    match prog.annotate_native_function(addr, label.as_deref(), comment.as_deref())
                    {
                        Ok(()) => format!(r#"{{"address":"0x{:x}","annotated":true}}"#, addr),
                        Err(e) => format!(
                            r#"{{"address":"0x{:x}","annotated":false,"error":{}}}"#,
                            addr,
                            escape_json(&e)
                        ),
                    }
                }
                _ => summary_json.to_string(),
            };
            Some(wrap_tool_text(&id_raw, &payload))
        }
        _ => Some(wrap_tool_text(&id_raw, summary_json)),
    }
}

pub fn run_stdio_mcp_loop(summary_json: &str) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let (mut prog, pseudo) = build_active_program();

    for line in stdin.lock().lines() {
        let line = line?;
        if let Some(response) = handle_mcp_line(&line, summary_json, &mut prog, &pseudo) {
            writeln!(stdout, "{}", response)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_dispatch_and_id_preservation() {
        let summary =
            r#"{"engine":"ghidra-hopper-rs","version":"0.3.1","rea":"6.10.1","ghidra":"12.3_DEV"}"#;
        let (mut prog, pseudo) = build_active_program();

        let init = handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":42,"method":"initialize"}"#,
            summary,
            &mut prog,
            &pseudo,
        )
        .unwrap();
        assert!(init.contains("\"id\":42"));
        assert!(init.contains("\"ghidra-hopper-rs-mcp\""));

        let list = handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":"req-2","method":"tools/list"}"#,
            summary,
            &mut prog,
            &pseudo,
        )
        .unwrap();
        assert!(list.contains("\"id\":\"req-2\""));
        assert!(list.contains("inspect_native_load_image"));
        assert!(list.contains("resolve_native_call_targets"));

        let call_targets = handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"resolve_native_call_targets","arguments":{"address":"0x10000000c"}}}"#,
            summary,
            &mut prog,
            &pseudo,
        )
        .unwrap();
        assert!(call_targets.contains("\"id\":9"));
        assert!(call_targets.contains("CallOverrideUnconditional"));
    }
}
