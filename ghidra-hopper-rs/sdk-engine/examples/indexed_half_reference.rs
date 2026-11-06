use ghidra_hopper_rs::{
    arm64_indexed::address,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable, Varnode},
    segment::Segment,
};
use std::{collections::BTreeMap, io::Read};
fn read(v: Varnode, regs: &[u64; 32], temps: &BTreeMap<u64, u64>) -> u64 {
    let value = match v.space {
        SpaceKind::Constant => v.offset,
        SpaceKind::Register => regs[v.offset as usize],
        SpaceKind::Unique => temps[&v.offset],
        _ => panic!("unsupported address space"),
    };
    value
        & if v.size == 8 {
            u64::MAX
        } else {
            (1u64 << (v.size * 8)) - 1
        }
}
fn write(v: Varnode, value: u64, regs: &mut [u64; 32], temps: &mut BTreeMap<u64, u64>) {
    let value = value
        & if v.size == 8 {
            u64::MAX
        } else {
            (1u64 << (v.size * 8)) - 1
        };
    match v.space {
        SpaceKind::Register => regs[v.offset as usize] = value,
        SpaceKind::Unique => {
            temps.insert(v.offset, value);
        }
        _ => panic!("unsupported destination"),
    }
}
fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&input).unwrap();
    let mut output = Vec::new();
    for row in rows.as_array().unwrap() {
        let word = u32::try_from(row["word"].as_u64().unwrap()).unwrap();
        let bytes: Vec<u8> = row["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
            .collect();
        let mut regs = [0u64; 32];
        let mut temps = BTreeMap::new();
        let rn = ((word >> 5) & 31) as usize;
        let rm = ((word >> 16) & 31) as usize;
        regs[rn] = row["base"].as_u64().unwrap();
        if rm != 31 {
            regs[rm] = row["index"].as_u64().unwrap();
        }
        let memory_base = row["memory_base"].as_u64().unwrap();
        let mut effective = 0;
        let mut loaded = 0;
        let mut reads = 0;
        let insn = decode_and_lift_arm64(
            0x1000,
            word,
            &Segment::new("probe", 0x1000, 0, vec![0; 16]),
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        for op in insn.pcode {
            match op {
                PcodeOp::RegisterOffset {
                    dst,
                    base,
                    index,
                    extend,
                    shift,
                } => {
                    effective = address(
                        read(base, &regs, &temps),
                        read(index, &regs, &temps),
                        extend,
                        shift,
                    )
                    .unwrap();
                    write(dst, effective, &mut regs, &mut temps);
                }
                PcodeOp::Load { dst, addr } => {
                    let offset = usize::try_from(
                        read(addr, &regs, &temps).checked_sub(memory_base).unwrap(),
                    )
                    .unwrap();
                    let raw = bytes
                        .get(offset..offset.checked_add(2).unwrap())
                        .expect("load outside probe memory");
                    loaded = u16::from_le_bytes(raw.try_into().unwrap()) as u64;
                    reads += 1;
                    write(dst, loaded, &mut regs, &mut temps);
                }
                PcodeOp::ZeroExtend { dst, src } => {
                    let value = read(src, &regs, &temps);
                    write(dst, value, &mut regs, &mut temps);
                }
                _ => panic!("unexpected LDRH operation"),
            }
        }
        let rt = (word & 31) as usize;
        output.push(serde_json::json!({"address":effective,"loaded":loaded,"reads":reads,"destination":if rt==31 {None}else{Some(regs[rt])}}));
    }
    println!("{}", serde_json::Value::Array(output));
}
