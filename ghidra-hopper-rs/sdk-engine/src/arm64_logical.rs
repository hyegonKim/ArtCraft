//! ORR integer data operations. No NZCV write occurs in either encoding.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalShift {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

pub fn evaluate(lhs: u64, rhs: u64, width: u8, shift: LogicalShift, amount: u8) -> Option<u64> {
    if !matches!(width, 32 | 64) || amount >= width {
        return None;
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        u32::MAX as u64
    };
    let rhs = rhs & mask;
    let shifted = match shift {
        LogicalShift::Lsl => {
            crate::arm64_flags::shifted(rhs, width, crate::arm64_flags::Shift::Lsl, amount)?
        }
        LogicalShift::Lsr => {
            crate::arm64_flags::shifted(rhs, width, crate::arm64_flags::Shift::Lsr, amount)?
        }
        LogicalShift::Asr => {
            crate::arm64_flags::shifted(rhs, width, crate::arm64_flags::Shift::Asr, amount)?
        }
        LogicalShift::Ror => {
            if amount == 0 {
                rhs
            } else {
                (rhs >> amount) | (rhs << (width - amount))
            }
        }
    };
    Some((lhs | shifted) & mask)
}

/// Decode logical-immediate bitmasks: rotated runs of ones, replicated to width.
/// Unlike bitfield masks, all-ones elements are reserved for these encodings.
pub fn immediate_mask(width: u8, n: u8, immr: u8, imms: u8) -> Option<u64> {
    if !matches!(width, 32 | 64) || n > 1 || immr > 63 || imms > 63 || (width == 32 && n != 0) {
        return None;
    }
    let selector = ((n as u32) << 6) | ((!imms as u32) & 63);
    if selector < 2 {
        return None;
    }
    let len = 31 - selector.leading_zeros();
    let element_width = 1u8 << len;
    if element_width > width {
        return None;
    }
    let levels = element_width - 1;
    let ones = imms & levels;
    let rotation = immr & levels;
    if ones == levels {
        return None;
    }
    let run = (1u64 << (ones + 1)) - 1;
    let element_mask = if element_width == 64 {
        u64::MAX
    } else {
        (1u64 << element_width) - 1
    };
    let element = if rotation == 0 {
        run
    } else {
        ((run >> rotation) | (run << (element_width - rotation))) & element_mask
    };
    let mut result = 0;
    for offset in (0..width).step_by(element_width as usize) {
        result |= element << offset;
    }
    Some(result)
}

fn read_reg(index: u64, size: u8) -> Varnode {
    if index == 31 {
        Varnode::imm(0, size)
    } else {
        Varnode::reg(index, size)
    }
}
fn reg_name(index: u64, size: u8) -> String {
    if index == 31 {
        if size == 8 { "xzr" } else { "wzr" }.into()
    } else {
        Varnode::reg(index, size).display_arm64()
    }
}
pub fn decode(address: u64, word: u32) -> Option<DecodedInstruction> {
    let immediate = word & 0x7f800000 == 0x32000000;
    let register = word & 0x7f200000 == 0x2a000000;
    if !(immediate || register) {
        return None;
    }
    let size = if word >> 31 != 0 { 8 } else { 4 };
    let rd = (word & 31) as u64;
    let rn = ((word >> 5) & 31) as u64;
    let (rhs, shift, amount, rhs_args) = if immediate {
        let value = immediate_mask(
            size * 8,
            ((word >> 22) & 1) as u8,
            ((word >> 16) & 63) as u8,
            ((word >> 10) & 63) as u8,
        )?;
        (
            Varnode::imm(value, size),
            LogicalShift::Lsl,
            0,
            vec![format!("#0x{value:x}")],
        )
    } else {
        let amount = ((word >> 10) & 63) as u8;
        if amount >= size * 8 {
            return None;
        }
        let rm = ((word >> 16) & 31) as u64;
        let shift = match (word >> 22) & 3 {
            0 => LogicalShift::Lsl,
            1 => LogicalShift::Lsr,
            2 => LogicalShift::Asr,
            _ => LogicalShift::Ror,
        };
        let mut args = vec![reg_name(rm, size)];
        if amount != 0 {
            args.push(format!(
                "{} #{amount}",
                match shift {
                    LogicalShift::Lsl => "lsl",
                    LogicalShift::Lsr => "lsr",
                    LogicalShift::Asr => "asr",
                    LogicalShift::Ror => "ror",
                }
            ));
        }
        (read_reg(rm, size), shift, amount, args)
    };
    // Logical immediate Rd=31 writes SP; shifted register Rd=31 discards to ZR.
    let dst = if !immediate && rd == 31 {
        Varnode {
            space: SpaceKind::Unique,
            offset: 4,
            size,
        }
    } else {
        Varnode::reg(rd, size)
    };
    let destination = if immediate {
        dst.display_arm64()
    } else {
        reg_name(rd, size)
    };
    let mut args = vec![destination, reg_name(rn, size)];
    args.extend(rhs_args);
    Some(DecodedInstruction {
        address,
        bytes: word.to_le_bytes(),
        mnemonic: "orr".into(),
        raw_args: args.clone(),
        formatted_args: args,
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![PcodeOp::IntOr {
            dst,
            lhs: read_reg(rn, size),
            rhs,
            shift,
            amount,
        }],
    })
}
