use ghidra_hopper_rs::{
    arm64_flags::arithmetic,
    arm64_logical::{evaluate, LogicalShift},
};
fn main() {
    let mut rows = Vec::new();
    for (name, width, shift, amount, immediate) in [
        ("orr_plain64", 64, LogicalShift::Lsl, 0, None),
        ("orr_lsl32", 32, LogicalShift::Lsl, 7, None),
        ("orr_lsl64", 64, LogicalShift::Lsl, 63, None),
        ("orr_lsr32", 32, LogicalShift::Lsr, 31, None),
        ("orr_lsr64", 64, LogicalShift::Lsr, 4, None),
        ("orr_asr32", 32, LogicalShift::Asr, 31, None),
        ("orr_asr64", 64, LogicalShift::Asr, 63, None),
        ("orr_ror32", 32, LogicalShift::Ror, 8, None),
        ("orr_ror64", 64, LogicalShift::Ror, 17, None),
        ("orr_imm32", 32, LogicalShift::Lsl, 0, Some(0xff00ff00u64)),
        (
            "orr_imm64",
            64,
            LogicalShift::Lsl,
            0,
            Some(0xff00ff00ff00ff00u64),
        ),
        ("orr_imm_rot32", 32, LogicalShift::Lsl, 0, Some(0xf8000007)),
        (
            "orr_imm_rot64",
            64,
            LogicalShift::Lsl,
            0,
            Some(0xf800000000000007),
        ),
    ] {
        let mask = if width == 64 {
            u64::MAX
        } else {
            u32::MAX as u64
        };
        let sign = 1u64 << (width - 1);
        for a in [0, 1, sign - 1, sign, mask, 0x8123456789abcdef & mask] {
            for b in [0, 1, sign - 1, sign, mask, 0xfedcba9876543210 & mask] {
                rows.push(serde_json::json!({"name":name,"width":width,"a":a,"b":b,"expected":evaluate(a,immediate.unwrap_or(b),width,shift,amount).unwrap(),"nzcv_before":arithmetic(a,b,width,true).unwrap().1.bits()}));
            }
        }
    }
    println!("{}", serde_json::Value::Array(rows));
}
