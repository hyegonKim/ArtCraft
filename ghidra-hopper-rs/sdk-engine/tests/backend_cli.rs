use serde_json::Value;
use std::process::Command;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn optional_provider_is_explicit_and_errors_are_structured() {
    let output = Command::new(env!("CARGO_BIN_EXE_ghidra-hopper-rs"))
        .arg("ghidra-decompile")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "error");
    assert_eq!(result["code"], "usage");
    assert_eq!(result["provider"], "external_ghidra");
}

#[test]
fn default_demo_remains_the_existing_rust_lifter() {
    let output = Command::new(env!("CARGO_BIN_EXE_ghidra-hopper-rs"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Hopper"));
    assert!(!text.contains("external_ghidra"));
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
#[test]
fn external_provider_is_an_explicit_typed_unsupported_host_on_other_platforms() {
    let output = Command::new(env!("CARGO_BIN_EXE_ghidra-hopper-rs"))
        .arg("ghidra-decompile")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "error");
    assert_eq!(result["code"], "unsupported_host");
    assert_eq!(result["provider"], "external_ghidra");
}
