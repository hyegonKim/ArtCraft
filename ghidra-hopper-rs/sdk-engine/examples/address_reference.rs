use ghidra_hopper_rs::{
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable},
    segment::Segment,
};
use std::{collections::BTreeMap, io::Read};
fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&input).unwrap();
    let mut output = Vec::new();
    for row in rows.as_array().unwrap() {
        let pc = row["pc"].as_u64().unwrap();
        let word = u32::try_from(row["word"].as_u64().unwrap()).unwrap();
        let insn = decode_and_lift_arm64(
            pc,
            word,
            &Segment::new("probe", pc, 0, vec![0; 16]),
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        let PcodeOp::Copy { dst, src } = insn.pcode[0] else {
            panic!("not an address-formation instruction")
        };
        assert_eq!(src.space, SpaceKind::Constant);
        assert_eq!(insn.pcode.len(), 1);
        output.push(serde_json::json!({"value":src.offset,"discard":dst.space!=SpaceKind::Register,"mnemonic":insn.mnemonic}));
    }
    println!("{}", serde_json::Value::Array(output));
}
