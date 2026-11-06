//! Hopper 6.0-style Built-in STDIO MCP Server (`HopperMCPServer` equivalent in Rust).
//!
//! Allows LLMs (Claude, Gemini, Cursor, etc.) to query the disassembled document,
//! P-Code decompiler, and Objective-C/Swift metadata over STDIO JSON-RPC.

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "error":{"code":code,"message":message}})
}

fn tool_result(value: Value) -> Value {
    json!({"content":[{"type":"text","text":value.to_string()}],"isError":false})
}

fn handle_request(line: &str, summary: &Value) -> Option<Value> {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => return Some(rpc_error(Value::Null, -32700, "Parse error")),
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if !request.is_object()
        || request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !request.get("method").is_some_and(Value::is_string)
        || !(id.is_null() || id.is_string() || id.is_number())
    {
        return Some(rpc_error(Value::Null, -32600, "Invalid Request"));
    }
    request.get("id")?;
    if request
        .get("params")
        .is_some_and(|p| !p.is_object() && !p.is_array())
    {
        return Some(rpc_error(id, -32602, "Invalid params"));
    }
    let result = match request["method"].as_str().unwrap() {
        "initialize" => json!({"protocolVersion":"2024-11-05",
            "serverInfo":{"name":"ghidra-hopper-rs-mcp","version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"tools":{}}}),
        "ping" => json!({}),
        "tools/list" => json!({"tools":[
            {"name":"binary_overview","description":"Return engine capabilities; no binary document is loaded","inputSchema":{"type":"object","additionalProperties":false}},
            {"name":"provider_status","description":"Return engine scope and explicitly unchecked provider health","inputSchema":{"type":"object","additionalProperties":false}},
            {"name":"demangle_swift","description":"Decode length-prefixed Swift identifiers","inputSchema":{"type":"object","properties":{"symbol":{"type":"string"}},"required":["symbol"],"additionalProperties":false}}
        ]}),
        "tools/call" => {
            let params = &request["params"];
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            if !args.is_object() {
                return Some(rpc_error(id, -32602, "Tool arguments must be an object"));
            }
            match params.get("name").and_then(Value::as_str) {
                Some("binary_overview" | "provider_status") => {
                    if !args.as_object().unwrap().is_empty() {
                        return Some(rpc_error(id, -32602, "This tool accepts no arguments"));
                    }
                    tool_result(summary.clone())
                }
                Some("demangle_swift") => {
                    let Some(symbol) = args.get("symbol").and_then(Value::as_str) else {
                        return Some(rpc_error(id, -32602, "symbol must be a string"));
                    };
                    if !(symbol.starts_with("$s") || symbol.starts_with("_$s")) {
                        return Some(rpc_error(
                            id,
                            -32602,
                            "Only Swift $s / _$s identifier extraction is supported",
                        ));
                    }
                    if args.as_object().unwrap().len() != 1 {
                        return Some(rpc_error(id, -32602, "Unexpected tool argument"));
                    }
                    tool_result(
                        json!({"symbol":symbol,"demangled":crate::segment::demangle_symbol(symbol),"status":"partial_identifier_extraction","full_demangling":false}),
                    )
                }
                _ => return Some(rpc_error(id, -32602, "Unknown or unimplemented tool")),
            }
        }
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

pub fn run_stdio_mcp_loop(summary_json: &str) -> io::Result<()> {
    let summary: Value = serde_json::from_str(summary_json)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_request(trimmed, &summary) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
