use ghidra_hopper_rs::{
    arm64_indexed::{address, IndexExtend},
    pcode_sleigh::{decode_and_lift_arm64, PcodeOp, SpaceKind, SwitchHintTable, Varnode},
    segment::Segment,
};
use std::collections::BTreeMap;
fn decode(word: u32) -> ghidra_hopper_rs::pcode_sleigh::DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        word,
        &Segment::new("text", 0x1000, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}
fn word(option: u32, scaled: bool, rn: u32, rm: u32, rt: u32) -> u32 {
    0x78600800 | (rm << 16) | (option << 13) | ((scaled as u32) << 12) | (rn << 5) | rt
}

struct Machine {
    regs: [u64; 32],
    temps: BTreeMap<u64, u64>,
    bytes: Vec<u8>,
    reads: usize,
}
impl Machine {
    fn new() -> Self {
        Self {
            regs: [0; 32],
            temps: BTreeMap::new(),
            bytes: vec![0; 256],
            reads: 0,
        }
    }
    fn read(&self, v: Varnode) -> u64 {
        let value = match v.space {
            SpaceKind::Constant => v.offset,
            SpaceKind::Register => self.regs[v.offset as usize],
            SpaceKind::Unique => self.temps[&v.offset],
            _ => panic!("invalid space"),
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
            _ => panic!("invalid destination"),
        }
    }
    fn execute(&mut self, w: u32) -> Result<(), &'static str> {
        for op in decode(w).pcode {
            match op {
                PcodeOp::RegisterOffset {
                    dst,
                    base,
                    index,
                    extend,
                    shift,
                } => self.write(
                    dst,
                    address(self.read(base), self.read(index), extend, shift).unwrap(),
                ),
                PcodeOp::Load { dst, addr } => {
                    let offset = self.read(addr).checked_sub(0x1000).ok_or("unmapped")? as usize;
                    let raw = self
                        .bytes
                        .get(offset..offset.checked_add(2).ok_or("overflow")?)
                        .ok_or("unmapped")?;
                    let value = u16::from_le_bytes(raw.try_into().unwrap()) as u64;
                    self.reads += 1;
                    self.write(dst, value);
                }
                PcodeOp::ZeroExtend { dst, src } => self.write(dst, self.read(src)),
                _ => panic!("unexpected operation"),
            }
        }
        Ok(())
    }
}

#[test]
fn hopper_load_uses_scaled_x8_and_zero_extends_halfword() {
    let d = decode(0x78687b4a);
    assert_eq!(d.mnemonic, "ldrh");
    assert!(
        matches!(d.pcode[0],PcodeOp::RegisterOffset{base,index,extend:IndexExtend::Uxtx,shift:1,..} if base.offset==26 && index.offset==8 && index.size==8)
    );
    let mut m = Machine::new();
    m.regs[26] = 0x1040;
    m.regs[8] = 3;
    m.regs[10] = u64::MAX;
    m.bytes[0x46..0x48].copy_from_slice(&[0x34, 0xfe]);
    m.execute(0x78687b4a).unwrap();
    assert_eq!(m.regs[10], 0xfe34);
    assert_eq!(m.regs[26], 0x1040);
    assert_eq!(m.regs[8], 3);
    assert_eq!(m.reads, 1);
}

#[test]
fn extension_scale_and_wrap_match_widened_arithmetic() {
    for extend in [
        IndexExtend::Uxtw,
        IndexExtend::Uxtx,
        IndexExtend::Sxtw,
        IndexExtend::Sxtx,
    ] {
        for shift in 0..2 {
            for base in [0u64, 0x1000, u64::MAX] {
                for index in [
                    0u64,
                    1,
                    0x7fffffff,
                    0x80000000,
                    0xffffffff,
                    0x100000002,
                    0x8000000000000000,
                    u64::MAX,
                ] {
                    let integer = match extend {
                        IndexExtend::Uxtw => (index & 0xffffffff) as i128,
                        IndexExtend::Sxtw => {
                            let i = (index & 0xffffffff) as i128;
                            if i >= 1 << 31 {
                                i - (1 << 32)
                            } else {
                                i
                            }
                        }
                        IndexExtend::Uxtx => index as i128,
                        IndexExtend::Sxtx => {
                            let i = index as i128;
                            if index >> 63 != 0 {
                                i - (1i128 << 64)
                            } else {
                                i
                            }
                        }
                    };
                    assert_eq!(
                        address(base, index, extend, shift),
                        Some((base as i128 + integer * (1i128 << shift)) as u64)
                    );
                }
            }
        }
    }
    assert_eq!(address(0, 1, IndexExtend::Uxtx, 2), None);
}

#[test]
fn signed_and_zero_extended_indexes_load_from_correct_bytes() {
    for (option, index, position) in [
        (2, 0xffff000000000002, 0x44),
        (6, 0xfffffffffffffffe, 0x3c),
        (7, u64::MAX - 1, 0x3c),
        (3, 2, 0x44),
    ] {
        let mut m = Machine::new();
        m.regs[0] = 0x1040;
        m.regs[1] = index;
        m.bytes[position..position + 2].copy_from_slice(&[0xff, 0xff]);
        m.execute(word(option, true, 0, 1, 2)).unwrap();
        assert_eq!(m.regs[2], 0xffff);
        assert_eq!(m.regs[0], 0x1040);
    }
}

#[test]
fn base_and_index_aliases_are_snapshotted_before_destination_write() {
    for (rn, rm, rt) in [(0, 1, 0), (0, 1, 1)] {
        let mut m = Machine::new();
        m.regs[0] = 0x1040;
        m.regs[1] = 2;
        m.bytes[0x44..0x46].copy_from_slice(&[0xcd, 0xab]);
        m.execute(word(3, true, rn, rm, rt)).unwrap();
        assert_eq!(m.regs[rt as usize], 0xabcd);
    }
}

#[test]
fn sp_zero_index_and_discarded_load_keep_memory_access() {
    let mut m = Machine::new();
    m.regs[31] = 0x1040;
    m.bytes[0x40..0x42].copy_from_slice(&[1, 0x80]);
    m.execute(word(3, true, 31, 31, 0)).unwrap();
    assert_eq!(m.regs[0], 0x8001);
    assert_eq!(m.regs[31], 0x1040);
    m.execute(word(3, false, 31, 31, 31)).unwrap();
    assert_eq!(m.reads, 2);
    assert_eq!(m.regs[31], 0x1040);
    m.regs[31] = 0;
    assert_eq!(m.execute(word(3, false, 31, 31, 31)), Err("unmapped"));
}

#[test]
fn reserved_extensions_and_different_load_kinds_are_not_accepted() {
    for option in [0, 1, 4, 5] {
        assert!(matches!(
            decode(word(option, false, 0, 1, 2)).pcode[0],
            PcodeOp::Unsupported { .. }
        ));
    }
    for w in [0xf8a16800, 0x38a16800, 0x78a16800, 0x7c210800] {
        assert!(
            !matches!(decode(w).pcode[0], PcodeOp::RegisterOffset { .. }),
            "{w:x}"
        );
    }
    let d = decode(0x78687b4a);
    assert_eq!(d.pcode.len(), 3);
    assert!(d
        .pcode
        .iter()
        .all(|p| !matches!(p, PcodeOp::FlagArithmetic { .. } | PcodeOp::ClobberNzcv)));
}
