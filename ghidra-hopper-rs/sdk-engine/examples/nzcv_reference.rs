use ghidra_hopper_rs::arm64_flags::{arithmetic, Condition};
fn main() {
    let mut rows = Vec::new();
    for width in [32u8, 64] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            u32::MAX as u64
        };
        let sign = 1u64 << (width - 1);
        for a in [0, 1, sign - 1, sign, sign + 1, mask] {
            for b in [0, 1, sign - 1, sign, sign + 1, mask] {
                for subtract in [false, true] {
                    let (result, flags) = arithmetic(a, b, width, subtract).unwrap();
                    let conditions: Vec<_> = (0..16)
                        .map(|c| Condition::from_code(c).unwrap().holds(flags))
                        .collect();
                    rows.push(serde_json::json!({"width":width,"a":a,"b":b,"subtract":subtract,"result":result,"nzcv":flags.bits(),"conditions":conditions}));
                }
            }
        }
    }
    println!("{}", serde_json::Value::Array(rows));
}
