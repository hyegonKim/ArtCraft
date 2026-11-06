//! Move-wide, conditional select, and explicitly modeled thread/acquire reads.
use crate::{
    arm64_flags::{Condition, Nzcv},
    pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode},
};

fn read(r: u64, size: u8) -> Varnode {
    if r == 31 {
        Varnode::imm(0, size)
    } else {
        Varnode::reg(r, size)
    }
}
fn write(r: u64, size: u8) -> Varnode {
    if r == 31 {
        Varnode {
            space: SpaceKind::Unique,
            offset: 2,
            size,
        }
    } else {
        Varnode::reg(r, size)
    }
}
fn name(r: u64, size: u8) -> String {
    if r == 31 {
        if size == 8 { "xzr" } else { "wzr" }.into()
    } else {
        Varnode::reg(r, size).display_arm64()
    }
}
pub fn movk_value(old: u64, immediate: u16, shift: u8, width: u8) -> Option<u64> {
    if !matches!(width, 32 | 64) || !shift.is_multiple_of(16) || shift >= width {
        return None;
    }
    let value = (old & !(0xffffu64 << shift)) | ((immediate as u64) << shift);
    Some(if width == 32 {
        value & u32::MAX as u64
    } else {
        value
    })
}
pub fn select_value(a: u64, b: u64, width: u8, condition: Condition, flags: Nzcv) -> Option<u64> {
    if !matches!(width, 32 | 64) {
        return None;
    }
    let value = if condition.holds(flags) { a } else { b };
    Some(if width == 32 {
        value & u32::MAX as u64
    } else {
        value
    })
}
pub fn sign_extend_word(value: u64) -> u64 {
    value as u32 as i32 as i64 as u64
}

pub fn decode(pc: u64, word: u32) -> Option<DecodedInstruction> {
    let mut i = DecodedInstruction {
        address: pc,
        bytes: word.to_le_bytes(),
        mnemonic: String::new(),
        raw_args: vec![],
        formatted_args: vec![],
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![],
    };
    let size = if word >> 31 != 0 { 8 } else { 4 };
    let rd = (word & 31) as u64;
    if word & 0x1f800000 == 0x12800000 {
        let opc = (word >> 29) & 3;
        let shift = (((word >> 21) & 3) * 16) as u8;
        if opc == 1 || shift >= size * 8 {
            return None;
        }
        let immediate = ((word >> 5) & 0xffff) as u16;
        let dst = write(rd, size);
        let value = (immediate as u64) << shift;
        if opc == 3 {
            i.mnemonic = "movk".into();
            i.pcode = vec![PcodeOp::MoveKeep {
                dst,
                old: read(rd, size),
                immediate,
                shift,
            }];
        } else {
            let value = if opc == 0 { !value } else { value }
                & if size == 4 { u32::MAX as u64 } else { u64::MAX };
            i.mnemonic = if opc == 0 { "movn" } else { "mov" }.into();
            i.pcode = vec![PcodeOp::Copy {
                dst,
                src: Varnode::imm(value, size),
            }];
            if opc == 2 {
                i.raw_args = vec![name(rd, size), format!("#0x{value:x}")];
            }
        }
        if i.raw_args.is_empty() {
            i.raw_args = vec![name(rd, size), format!("#0x{immediate:x}")];
            if shift != 0 {
                i.raw_args.push(format!("lsl #{shift}"));
            }
        }
    } else if word & 0x7fe00c00 == 0x1a800000 {
        let rn = ((word >> 5) & 31) as u64;
        let rm = ((word >> 16) & 31) as u64;
        let condition = Condition::from_code(((word >> 12) & 15) as u8)?;
        i.mnemonic = "csel".into();
        i.raw_args = vec![
            name(rd, size),
            name(rn, size),
            name(rm, size),
            condition.name().into(),
        ];
        i.pcode = vec![PcodeOp::ConditionalSelect {
            dst: write(rd, size),
            if_true: read(rn, size),
            if_false: read(rm, size),
            condition,
        }];
    } else if word & 0xffffffe0 == 0xd53bd040 {
        i.mnemonic = "mrs".into();
        i.raw_args = vec![name(rd, 8), "TPIDR_EL0".into()];
        i.pcode = vec![PcodeOp::ReadThreadPointer { dst: write(rd, 8) }];
    } else if word & 0xbffffc00 == 0x88dffc00 && rd != 31 {
        let rn = ((word >> 5) & 31) as u64;
        let size = if word & (1 << 30) != 0 { 8 } else { 4 };
        i.mnemonic = "ldar".into();
        i.raw_args = vec![
            name(rd, size),
            format!("[{}]", Varnode::reg(rn, 8).display_arm64()),
        ];
        i.pcode = vec![PcodeOp::LoadAcquire {
            dst: Varnode::reg(rd, size),
            addr: Varnode::reg(rn, 8),
        }];
    } else {
        return None;
    }
    i.formatted_args = i.raw_args.clone();
    Some(i)
}
