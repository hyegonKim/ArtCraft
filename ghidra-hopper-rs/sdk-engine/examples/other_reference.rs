//! Single-instruction differential oracle used by validation/other_entries.
use ghidra_hopper_rs::{
    arm64_data::{movk_value, select_value, sign_extend_word},
    arm64_flags::Nzcv,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable, Varnode},
    segment::Segment,
};
use std::{collections::BTreeMap, io::Read};
struct Machine {
    regs: [u64; 32],
    temps: BTreeMap<u64, u64>,
    base: u64,
    bytes: Vec<u8>,
    reads: usize,
    acquires: usize,
    thread: u64,
    flags: Nzcv,
}
impl Machine {
    fn read(&self, v: Varnode) -> u64 {
        let value = match v.space {
            SpaceKind::Register => self.regs[v.offset as usize],
            SpaceKind::Unique => self.temps[&v.offset],
            SpaceKind::Constant => v.offset,
            SpaceKind::Ram | SpaceKind::Stack => panic!("unexpected value address space"),
        };
        value
            & if v.size == 8 {
                u64::MAX
            } else {
                (1u64 << (v.size * 8)) - 1
            }
    }
    fn write(&mut self, v: Varnode, value: u64) {
        let value = value
            & if v.size == 8 {
                u64::MAX
            } else {
                (1u64 << (v.size * 8)) - 1
            };
        match v.space {
            SpaceKind::Register => self.regs[v.offset as usize] = value,
            SpaceKind::Unique => {
                self.temps.insert(v.offset, value);
            }
            SpaceKind::Constant | SpaceKind::Ram | SpaceKind::Stack => {
                panic!("unexpected output address space")
            }
        }
    }
    fn load(&mut self, address: u64, size: u8) -> u64 {
        self.reads += 1;
        let offset = usize::try_from(address.checked_sub(self.base).unwrap()).unwrap();
        let bytes = self.bytes.get(offset..offset + size as usize).unwrap();
        bytes
            .iter()
            .enumerate()
            .fold(0, |v, (n, b)| v | ((*b as u64) << (n * 8)))
    }
    fn execute(&mut self, op: &PcodeOp) {
        match op {
            PcodeOp::Copy { dst, src } => self.write(*dst, self.read(*src)),
            PcodeOp::IntAdd { dst, lhs, rhs } => {
                self.write(*dst, self.read(*lhs).wrapping_add(self.read(*rhs)))
            }
            PcodeOp::MoveKeep {
                dst,
                old,
                immediate,
                shift,
            } => self.write(
                *dst,
                movk_value(self.read(*old), *immediate, *shift, dst.size * 8).unwrap(),
            ),
            PcodeOp::ConditionalSelect {
                dst,
                if_true,
                if_false,
                condition,
            } => self.write(
                *dst,
                select_value(
                    self.read(*if_true),
                    self.read(*if_false),
                    dst.size * 8,
                    *condition,
                    self.flags,
                )
                .unwrap(),
            ),
            PcodeOp::Load { dst, addr } => {
                let value = self.load(self.read(*addr), dst.size);
                self.write(*dst, value);
            }
            PcodeOp::SignExtend { dst, src } => self.write(*dst, sign_extend_word(self.read(*src))),
            PcodeOp::LoadAcquire { dst, addr } => {
                let value = self.load(self.read(*addr), dst.size);
                self.acquires += 1;
                self.write(*dst, value);
            }
            PcodeOp::ReadThreadPointer { dst } => self.write(*dst, self.thread),
            _ => panic!("unexpected instruction: {op:?}"),
        }
    }
}
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut output = Vec::new();
    for row in rows.as_array().unwrap() {
        let regs: Vec<u64> = row["regs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let mut m = Machine {
            regs: regs.try_into().unwrap(),
            temps: BTreeMap::new(),
            base: row["memory_base"].as_u64().unwrap_or(0),
            bytes: row["bytes"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
                        .collect()
                })
                .unwrap_or_default(),
            reads: 0,
            acquires: 0,
            thread: row["thread_pointer"].as_u64().unwrap_or(0),
            flags: Nzcv::from_bits(row["flags"].as_u64().unwrap_or(0) as u8),
        };
        let word = u32::try_from(row["word"].as_u64().unwrap()).unwrap();
        let i = decode_and_lift_arm64(
            0x1000,
            word,
            &Segment::new("probe", 0x1000, 0, vec![0; 16]),
            &BTreeMap::new(),
            &SwitchHintTable::default(),
        );
        for op in &i.pcode {
            m.execute(op);
        }
        output.push(serde_json::json!({"regs":m.regs.to_vec(),"reads":m.reads,"acquires":m.acquires,"flags":m.flags.bits()}));
    }
    println!("{}", serde_json::Value::Array(output));
}
