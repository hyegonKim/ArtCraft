use ghidra_hopper_rs::{
    pcode_sleigh::{
        decode_and_lift_arm64, DecodedInstruction, PcodeOp, SpaceKind, SwitchHintTable, Varnode,
    },
    segment::Segment,
};
use std::collections::BTreeMap;

fn decode(word: u32) -> DecodedInstruction {
    decode_and_lift_arm64(
        0x1000,
        word,
        &Segment::new("text", 0x1000, 0, vec![0; 16]),
        &BTreeMap::new(),
        &SwitchHintTable::default(),
    )
}

#[derive(Default)]
struct Machine {
    regs: [u64; 32],
    temps: BTreeMap<u64, u64>,
    bytes: BTreeMap<u64, u8>,
}
fn mask(size: u8) -> u64 {
    if size == 8 {
        u64::MAX
    } else {
        (1u64 << (size * 8)) - 1
    }
}
impl Machine {
    fn read(&self, v: Varnode) -> u64 {
        (match v.space {
            SpaceKind::Register => self.regs[v.offset as usize],
            SpaceKind::Constant => v.offset,
            SpaceKind::Unique => self.temps[&v.offset],
            _ => panic!("unexpected space"),
        }) & mask(v.size)
    }
    fn write(&mut self, v: Varnode, value: u64) {
        let value = value & mask(v.size);
        match v.space {
            SpaceKind::Register => self.regs[v.offset as usize] = value,
            SpaceKind::Unique => {
                self.temps.insert(v.offset, value);
            }
            _ => panic!("invalid destination"),
        }
    }
    fn execute(&mut self, word: u32) {
        for op in decode(word).pcode {
            match op {
                PcodeOp::Copy { dst, src } | PcodeOp::ZeroExtend { dst, src } => {
                    self.write(dst, self.read(src))
                }
                PcodeOp::IntAdd { dst, lhs, rhs } => {
                    self.write(dst, self.read(lhs).wrapping_add(self.read(rhs)))
                }
                PcodeOp::Load { dst, addr } => {
                    let a = self.read(addr);
                    let value = (0..dst.size)
                        .map(|i| (*self.bytes.get(&(a + i as u64)).unwrap_or(&0) as u64) << (8 * i))
                        .fold(0, |a, b| a | b);
                    self.write(dst, value);
                }
                PcodeOp::Store { addr, src } => {
                    let a = self.read(addr);
                    let value = self.read(src);
                    for i in 0..src.size {
                        self.bytes.insert(a + i as u64, (value >> (8 * i)) as u8);
                    }
                }
                PcodeOp::Bti { .. } => {}
                PcodeOp::Return { .. } => {}
                other => panic!("unexpected operation: {other:?}"),
            }
        }
    }
}

#[test]
fn pair_preindex_store_and_postindex_load_round_trip() {
    let mut m = Machine::default();
    m.regs[31] = 0x2000;
    m.regs[29] = 0x123456789abcdef0;
    m.regs[30] = 0xfedcba9876543210;
    m.execute(0xa9bf7bfd); // stp fp, lr, [sp, #-16]!
    assert_eq!(m.regs[31], 0x1ff0);
    assert_eq!(m.bytes[&0x1ff0], 0xf0);
    assert_eq!(m.bytes[&0x1ff8], 0x10);
    m.regs[29] = 0;
    m.regs[30] = 0;
    m.execute(0xa8c17bfd); // ldp fp, lr, [sp], #16
    assert_eq!(m.regs[31], 0x2000);
    assert_eq!(m.regs[29], 0x123456789abcdef0);
    assert_eq!(m.regs[30], 0xfedcba9876543210);
    assert!(
        matches!(decode(0xa9bf7bfd).pcode.last(),Some(PcodeOp::IntAdd{dst,..}) if dst.offset==31 && dst.space==SpaceKind::Register)
    );
}

#[test]
fn hopper_pair_offset_has_no_writeback() {
    let mut m = Machine::default();
    m.regs[31] = 0x1000;
    m.regs[28] = 42;
    m.regs[27] = 99;
    m.execute(0xa9096ffc); // stp x28, x27, [sp, #144]
    assert_eq!(m.regs[31], 0x1000);
    assert_eq!(m.bytes[&0x1090], 42);
    assert_eq!(m.bytes[&0x1098], 99);
}

#[test]
fn offset_load_snapshots_base_when_first_destination_is_base() {
    let mut m = Machine::default();
    m.regs[0] = 0x1000;
    m.bytes.insert(0x1000, 20);
    m.bytes.insert(0x1008, 30);
    m.execute(0xa9400400); // ldp x0, x1, [x0]
    assert_eq!(m.regs[0], 20);
    assert_eq!(m.regs[1], 30);
}

#[test]
fn pair_word_loads_zero_extend_and_zero_register_is_not_sp() {
    let mut m = Machine::default();
    m.regs[31] = 0x2000;
    m.regs[0] = u64::MAX;
    for i in 0..8 {
        m.bytes.insert(0x2000 + i, 0xff);
    }
    m.execute(0x294007e0); // ldp w0, w1, [sp]
    assert_eq!(m.regs[0], u32::MAX as u64);
    assert_eq!(m.regs[31], 0x2000);
    m.execute(0xa9007fff); // stp xzr, xzr, [sp]
    assert!((0..16).all(|i| m.bytes[&(0x2000 + i)] == 0));
    assert_eq!(m.regs[31], 0x2000);
}

#[test]
fn unsigned_loads_obey_scale_width_and_zero_extension() {
    let mut m = Machine::default();
    m.regs[8] = 0x2000;
    m.regs[0] = u64::MAX;
    m.bytes.insert(0x2200, 0x44);
    m.execute(0xf9410100); // ldr x0, [x8, #512]
    assert_eq!(m.regs[0], 0x44);
    m.bytes.insert(0x2000, 0xab);
    m.execute(0x39400100); // ldrb w0, [x8]
    assert_eq!(m.regs[0], 0xab);
    m.regs[0] = 0x1234;
    m.execute(0x39000500); // strb w0, [x8,#1]
    assert_eq!(m.bytes[&0x2001], 0x34);
}

#[test]
fn mov_zero_register_and_bti_are_explicit() {
    let mut m = Machine::default();
    m.regs[0] = u64::MAX;
    m.regs[31] = 0x2000;
    m.regs[1] = 7;
    m.execute(0xaa0003f7);
    assert_eq!(m.regs[23], u64::MAX);
    m.execute(0x2a0003e1);
    assert_eq!(m.regs[1], u32::MAX as u64);
    m.execute(0xaa1f03e1);
    assert_eq!(m.regs[1], 0);
    assert_eq!(m.regs[31], 0x2000);
    assert!(matches!(
        decode(0xd503245f).pcode[0],
        PcodeOp::Bti { kind: 1 }
    ));
    let insn = decode(0xd63f0100);
    assert_eq!(insn.mnemonic, "blr");
    assert!(insn.is_call);
}

#[test]
fn unsupported_and_unpredictable_variants_still_stop() {
    for word in [0xac000400, 0xa9c00400, 0xa9400000, 0x69400400, 0x39800000] {
        assert!(
            matches!(decode(word).pcode[0], PcodeOp::Unsupported { .. }),
            "{word:x}"
        );
    }
}

#[test]
fn signed_single_transfers_and_writeback() {
    let mut m = Machine::default();
    m.regs[29] = 0x2000;
    m.regs[0] = 0x12345678;
    m.execute(0xf81f83a0); // stur x0,[fp,#-8]
    assert_eq!(m.bytes[&0x1ff8], 0x78);
    assert_eq!(m.regs[29], 0x2000);
    m.execute(0xf85f83a1); // ldur x1,[fp,#-8]
    assert_eq!(m.regs[1], 0x12345678);
    m.regs[2] = 0x3000;
    m.execute(0xf81f8c40); // str x0,[x2,#-8]!
    assert_eq!(m.regs[2], 0x2ff8);
    assert_eq!(m.bytes[&0x2ff8], 0x78);
    m.execute(0xf8408443); // ldr x3,[x2],#8
    assert_eq!(m.regs[3], 0x12345678);
    assert_eq!(m.regs[2], 0x3000);
}

#[test]
fn compiled_native_main_ir_preserves_frame_and_returns_42() {
    let mut m = Machine::default();
    m.regs[31] = 0x2000;
    m.regs[29] = 0x1234;
    m.regs[30] = 0x5678;
    for word in [
        0xd503245f, 0xa9be7bfd, 0x910003fd, 0xd2800540, 0xf9000be0, 0xf9400be1, 0xaa0103e2,
        0xf80183a2, 0xf84183a0, 0xa8c27bfd, 0xd65f03c0,
    ] {
        m.execute(word);
    }
    assert_eq!(m.regs[0], 42);
    assert_eq!(m.regs[31], 0x2000);
    assert_eq!(m.regs[29], 0x1234);
    assert_eq!(m.regs[30], 0x5678);
}
