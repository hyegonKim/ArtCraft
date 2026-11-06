//! Bounded concrete validation model. Opaque ABI calls are not executed. The
//! monitor models address/size/version and explicitly requested spurious failure;
//! hardware may invalidate a reservation for additional implementation reasons.
use ghidra_hopper_rs::{
    arm64_atomic::add_value,
    arm64_flags::add_shifted,
    arm64_indexed::offset_address,
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable, Varnode},
    segment::Segment,
};
use std::{collections::BTreeMap, io::Read};
struct Machine {
    regs: [u64; 32],
    vectors: [u128; 32],
    temps: BTreeMap<u64, u64>,
    base: u64,
    bytes: Vec<u8>,
    version: u64,
    monitor: Option<(u64, u8, u64)>,
    failures: u64,
    interference: Option<u64>,
    attempts: usize,
    reads: usize,
    writes: usize,
    rmws: usize,
    flags: u8,
}
impl Machine {
    fn read(&self, v: Varnode) -> u64 {
        let value = match v.space {
            SpaceKind::Register => self.regs[v.offset as usize],
            SpaceKind::Constant => v.offset,
            SpaceKind::Unique => self.temps[&v.offset],
            _ => panic!("unsupported input space"),
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
            _ => panic!("unsupported output space"),
        }
    }
    fn load(&mut self, addr: u64, size: u8) -> u128 {
        self.reads += 1;
        let offset = usize::try_from(addr.checked_sub(self.base).unwrap()).unwrap();
        self.bytes
            .get(offset..offset + size as usize)
            .unwrap()
            .iter()
            .enumerate()
            .fold(0, |out, (n, b)| out | ((*b as u128) << (n * 8)))
    }
    fn store(&mut self, addr: u64, size: u8, value: u128) {
        self.writes += 1;
        self.version += 1;
        let offset = usize::try_from(addr.checked_sub(self.base).unwrap()).unwrap();
        let dest = self.bytes.get_mut(offset..offset + size as usize).unwrap();
        for (n, b) in dest.iter_mut().enumerate() {
            *b = (value >> (n * 8)) as u8;
        }
    }
    fn execute(&mut self, op: &PcodeOp, next: &mut u64, halt: &mut bool) {
        match op {
            PcodeOp::ConditionalCompare {
                lhs,
                rhs,
                width,
                condition,
                nzcv,
                subtract,
            } => {
                self.flags = ghidra_hopper_rs::arm64_integer::conditional_compare(
                    self.read(*lhs),
                    self.read(*rhs),
                    *width,
                    *condition,
                    *nzcv,
                    *subtract,
                    ghidra_hopper_rs::arm64_flags::Nzcv::from_bits(self.flags),
                )
                .unwrap()
                .bits();
            }
            PcodeOp::SignedMultiplyHigh { dst, lhs, rhs } => {
                let value = ghidra_hopper_rs::arm64_integer::signed_multiply_high(
                    self.read(*lhs),
                    self.read(*rhs),
                );
                if let Some(dst) = dst {
                    self.write(*dst, value);
                }
            }
            PcodeOp::Logical {
                dst,
                lhs,
                rhs,
                op,
                invert,
                shift,
                amount,
                width,
                set_flags,
            } => {
                let value = ghidra_hopper_rs::arm64_integer::logical(
                    self.read(*lhs),
                    self.read(*rhs),
                    *width,
                    *op,
                    *invert,
                    *shift,
                    *amount,
                )
                .unwrap();
                if let Some(dst) = dst {
                    self.write(*dst, value);
                }
                if *set_flags {
                    self.flags = ghidra_hopper_rs::arm64_integer::logical_flags(value, *width)
                        .unwrap()
                        .bits();
                }
            }
            PcodeOp::ConditionalTransform {
                dst,
                if_true,
                if_false,
                condition,
                action,
                width,
            } => {
                let value = ghidra_hopper_rs::arm64_integer::selected(
                    self.read(*if_true),
                    self.read(*if_false),
                    *width,
                    *condition,
                    ghidra_hopper_rs::arm64_flags::Nzcv::from_bits(self.flags),
                    *action,
                )
                .unwrap();
                if let Some(dst) = dst {
                    self.write(*dst, value);
                }
            }
            PcodeOp::RegisterArithmetic {
                dst,
                lhs,
                rhs,
                extension,
                shift,
                amount,
                width,
                subtract,
                set_flags,
            } => {
                let (value, flags) = ghidra_hopper_rs::arm64_integer::register_arithmetic(
                    self.read(*lhs),
                    self.read(*rhs),
                    *width,
                    *extension,
                    *shift,
                    *amount,
                    *subtract,
                    *set_flags,
                )
                .unwrap();
                if let Some(dst) = dst {
                    self.write(*dst, value);
                }
                if let Some(flags) = flags {
                    self.flags = flags.bits();
                }
            }
            PcodeOp::SignedMultiplyAddLong {
                dst,
                lhs,
                rhs,
                addend,
            } => {
                let value = ghidra_hopper_rs::arm64_integer::signed_multiply_add_long(
                    self.read(*lhs),
                    self.read(*rhs),
                    self.read(*addend),
                );
                if let Some(dst) = dst {
                    self.write(*dst, value);
                }
            }
            PcodeOp::MultiplyAdd {
                dst,
                lhs,
                rhs,
                addend,
                width,
                subtract,
            } => {
                let value = ghidra_hopper_rs::arm64_integer::multiply_add(
                    self.read(*lhs),
                    self.read(*rhs),
                    self.read(*addend),
                    *width,
                    *subtract,
                )
                .unwrap();
                if let Some(dst) = dst {
                    self.write(*dst, value);
                }
            }
            PcodeOp::VectorLoadSized {
                register,
                addr,
                size,
            } => {
                let value = self.load(self.read(*addr), *size);
                self.vectors[*register as usize] = value;
            }
            PcodeOp::VectorStoreSized {
                register,
                addr,
                size,
            } => {
                self.store(self.read(*addr), *size, self.vectors[*register as usize]);
            }
            PcodeOp::Copy { dst, src } => self.write(*dst, self.read(*src)),
            PcodeOp::IntAdd { dst, lhs, rhs } => {
                self.write(*dst, self.read(*lhs).wrapping_add(self.read(*rhs)))
            }
            PcodeOp::IntSub { dst, lhs, rhs } => {
                self.write(*dst, self.read(*lhs).wrapping_sub(self.read(*rhs)))
            }
            PcodeOp::ShiftedAdd {
                dst: Some(dst),
                lhs,
                rhs,
                shift,
                amount,
            } => self.write(
                *dst,
                add_shifted(
                    self.read(*lhs),
                    self.read(*rhs),
                    dst.size * 8,
                    *shift,
                    *amount,
                )
                .unwrap(),
            ),
            PcodeOp::RegisterOffset {
                dst,
                base,
                index,
                extend,
                shift,
            } => self.write(
                *dst,
                offset_address(self.read(*base), self.read(*index), *extend, *shift).unwrap(),
            ),
            PcodeOp::Load { dst, addr } => {
                let value = self.load(self.read(*addr), dst.size) as u64;
                self.write(*dst, value);
            }
            PcodeOp::Store { addr, src } => {
                self.store(self.read(*addr), src.size, self.read(*src) as u128)
            }
            PcodeOp::ZeroExtend { dst, src } => self.write(*dst, self.read(*src)),
            PcodeOp::SignExtend { dst, src } => {
                self.write(*dst, self.read(*src) as u32 as i32 as i64 as u64)
            }
            PcodeOp::VectorImmediate {
                register,
                low,
                high,
            } => self.vectors[*register as usize] = (*low as u128) | ((*high as u128) << 64),
            PcodeOp::VectorLoad { register, addr } => {
                let value = self.load(self.read(*addr), 16);
                self.vectors[*register as usize] = value;
            }
            PcodeOp::VectorStore { register, addr } => {
                self.store(self.read(*addr), 16, self.vectors[*register as usize])
            }
            PcodeOp::AtomicAdd { dst, src, addr, .. } => {
                let address = self.read(*addr);
                let addend = self.read(*src);
                assert_eq!(address % dst.size as u64, 0);
                let old = self.load(address, dst.size) as u64;
                let (previous, value) = add_value(old, addend, dst.size * 8).unwrap();
                self.store(address, dst.size, value as u128);
                self.rmws += 1;
                self.write(*dst, previous);
            }
            PcodeOp::ExclusiveLoadAcquire { dst, addr } => {
                let address = self.read(*addr);
                assert_eq!(address % dst.size as u64, 0);
                let value = self.load(address, dst.size) as u64;
                self.monitor = Some((address, dst.size, self.version));
                self.write(*dst, value);
                if let Some(other) = self.interference.take() {
                    self.store(address, dst.size, other as u128);
                }
            }
            PcodeOp::ExclusiveStoreRelease { status, src, addr } => {
                let address = self.read(*addr);
                let value = self.read(*src);
                assert_eq!(address % src.size as u64, 0);
                self.attempts += 1;
                let reservation = self.monitor.take();
                let forced = self.failures > 0;
                if forced {
                    self.failures -= 1;
                }
                let success = !forced && reservation == Some((address, src.size, self.version));
                if success {
                    self.store(address, src.size, value as u128);
                }
                self.write(*status, if success { 0 } else { 1 });
            }
            PcodeOp::ClearExclusive => self.monitor = None,
            PcodeOp::CBranch {
                cond,
                target,
                branch_on_zero,
            } => {
                if (self.read(*cond) == 0) == *branch_on_zero {
                    *next = *target;
                }
            }
            PcodeOp::Branch { target } => *next = *target,
            PcodeOp::FlagBranch { condition, target } => {
                if condition.holds(ghidra_hopper_rs::arm64_flags::Nzcv::from_bits(self.flags)) {
                    *next = *target;
                }
            }
            PcodeOp::Return { .. } => *halt = true,
            PcodeOp::Nop | PcodeOp::Bti { .. } => {}
            _ => panic!("unexpected op: {op:?}"),
        }
    }
}
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut result = Vec::new();
    for row in rows.as_array().unwrap() {
        let regs: Vec<u64> = row["regs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let mut vectors = [0u128; 32];
        if let Some(vs) = row["vectors"].as_object() {
            for (reg, value) in vs {
                vectors[reg.parse::<usize>().unwrap()] = (value[0].as_u64().unwrap() as u128)
                    | ((value[1].as_u64().unwrap() as u128) << 64);
            }
        }
        let mut m = Machine {
            regs: regs.try_into().unwrap(),
            vectors,
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
            version: 0,
            monitor: None,
            failures: row["forced_failures"].as_u64().unwrap_or(0),
            interference: row["interference"].as_u64(),
            attempts: 0,
            reads: 0,
            writes: 0,
            rmws: 0,
            flags: row["flags"].as_u64().unwrap_or(0) as u8,
        };
        let words: Vec<u32> = if let Some(a) = row["words"].as_array() {
            a.iter()
                .map(|v| u32::try_from(v.as_u64().unwrap()).unwrap())
                .collect()
        } else {
            vec![u32::try_from(row["word"].as_u64().unwrap()).unwrap()]
        };
        let segment = Segment::new(
            "probe",
            0x1000,
            0,
            words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        );
        let mut pc = 0x1000;
        let mut steps = 0;
        let mut halt = false;
        while !halt && pc < 0x1000 + words.len() as u64 * 4 {
            assert!(steps < 10000);
            let word = segment.read_u32_le(pc).unwrap();
            let i = decode_and_lift_arm64(
                pc,
                word,
                &segment,
                &BTreeMap::new(),
                &SwitchHintTable::default(),
            );
            let mut next = pc + 4;
            for op in &i.pcode {
                m.execute(op, &mut next, &mut halt);
            }
            pc = next;
            steps += 1;
        }
        let vectors: Vec<[u64; 2]> = m
            .vectors
            .iter()
            .map(|v| [*v as u64, (*v >> 64) as u64])
            .collect();
        result.push(serde_json::json!({"regs":m.regs.to_vec(),"vectors":vectors,"bytes":m.bytes,"flags":m.flags,"reads":m.reads,"writes":m.writes,"rmws":m.rmws,"attempts":m.attempts,"reservation":m.monitor.is_some(),"steps":steps}));
    }
    println!("{}", serde_json::Value::Array(result));
}
