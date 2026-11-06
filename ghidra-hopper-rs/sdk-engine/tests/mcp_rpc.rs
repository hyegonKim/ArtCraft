use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn exchange(lines: &[String]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ghidra-hopper-rs"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for line in lines {
        writeln!(input, "{line}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}

#[test]
fn rpc_preserves_ids_and_dispatches_only_the_method_field() {
    let responses = exchange(&[
        json!({"jsonrpc":"2.0","id":"init\"42","method":"initialize"}).to_string(),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
        json!({"jsonrpc":"2.0","id":71,"method":"ping","params":{"name":"tools/list"}}).to_string(),
        json!({"jsonrpc":"2.0","id":null,"method":"unknown","params":{"name":"initialize"}})
            .to_string(),
    ]);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["id"], "init\"42");
    assert_eq!(responses[1]["id"], 71);
    assert_eq!(responses[1]["result"], json!({}));
    assert_eq!(responses[2]["id"], Value::Null);
    assert_eq!(responses[2]["error"]["code"], -32601);
}

#[test]
fn rpc_reports_invalid_messages_and_keeps_running() {
    let responses = exchange(&[
        "{".into(),
        "[]".into(),
        json!({"jsonrpc":"2.0","id":{},"method":"ping"}).to_string(),
        json!({"jsonrpc":"2.0","id":4,"method":"ping","params":7}).to_string(),
        json!({"jsonrpc":"2.0","id":5,"method":"ping"}).to_string(),
    ]);
    for (response, code) in responses.iter().zip([-32700, -32600, -32600, -32602]) {
        assert_eq!(response["error"]["code"], code);
    }
    assert_eq!(responses.len(), 5);
    assert_eq!(responses[4]["result"], json!({}));
}

#[test]
fn tools_have_schemas_and_unsupported_actions_cannot_report_success() {
    let responses = exchange(&[
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"demangle_swift","arguments":{"symbol":"$s4Test"}}}).to_string(),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"decompile_objc"}}).to_string(),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"demangle_swift","arguments":{"symbol":7}}}).to_string(),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"provider_status"}}).to_string(),
    ]);
    let tools = responses[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 3);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
    let text = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text).unwrap()["demangled"],
        "swift::Test"
    );
    assert_eq!(responses[2]["error"]["code"], -32602);
    assert_eq!(responses[3]["error"]["code"], -32602);
    assert_eq!(responses[4]["result"]["content"][0]["type"], "text");
}
