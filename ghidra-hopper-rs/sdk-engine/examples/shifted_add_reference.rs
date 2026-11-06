use ghidra_hopper_rs::{
    arm64_flags::add_shifted,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable, Varnode},
    segment::Segment,
};
use std::{collections::BTreeMap, io::Read};
fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&input).unwrap();
    let mut output = Vec::new();
    for row in rows.as_array().unwrap() {
        let word = u32::try_from(row["word"].as_u64().unwrap()).unwrap();
        let a = row["a"].as_u64().unwrap();
        let b = row["b"].as_u64().unwrap();
        let read = |v: Varnode| {
            let value = if v.space == SpaceKind::Constant {
                v.offset
            } else {
                match v.offset {
                    0 => a,
                    1 => b,
                    31 => 0x777788889999aaaa,
                    _ => panic!("probe register not initialized"),
                }
            };
            value
                & if v.size == 8 {
                    u64::MAX
                } else {
                    u32::MAX as u64
                }
        };
        let insn = decode_and_lift_arm64(
            0x1000,
            word,
            &Segment::new("probe", 0x1000, 0, vec![0; 16]),
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        assert_eq!(insn.pcode.len(), 1);
        let PcodeOp::ShiftedAdd {
            dst,
            lhs,
            rhs,
            shift,
            amount,
        } = insn.pcode[0]
        else {
            panic!("not shifted ADD")
        };
        let value = add_shifted(read(lhs), read(rhs), lhs.size * 8, shift, amount).unwrap();
        output.push(serde_json::json!({"value":value,"discard":dst.is_none(),"width":lhs.size*8,"shift":format!("{shift:?}"),"amount":amount}));
    }
    println!("{}", serde_json::Value::Array(output));
}
