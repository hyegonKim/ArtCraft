use ghidra_hopper_rs::arm64_test_branch::taken;
fn main() {
    let mut rows = Vec::new();
    for bit in 0..64 {
        for nonzero in [false, true] {
            for value in [0, u64::MAX, 1u64 << bit, !(1u64 << bit), 0x8123456789abcdef] {
                rows.push(serde_json::json!({"name":format!("{}_{}",if nonzero {"tbnz"}else{"tbz"},bit),"value":value,"expected":taken(value,bit,nonzero).unwrap()}));
            }
        }
    }
    println!("{}", serde_json::Value::Array(rows));
}
