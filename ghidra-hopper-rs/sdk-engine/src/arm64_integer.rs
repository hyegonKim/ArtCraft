//! Additional integer operations. Inputs use ZR or SP according to the
//! encoding; W writes zero the upper half and logical flag writes clear C/V.
use crate::{
    arm64_flags::{arithmetic, shifted, Condition, Nzcv, Shift},
    arm64_logical::{immediate_mask, LogicalShift},
    pcode_sleigh::{DecodedInstruction, PcodeOp, Varnode},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logic {
    And,
    Or,
    Xor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectAction {
    Increment,
    Invert,
    Negate,
}
fn mask(width: u8) -> Option<u64> {
    match width {
        32 => Some(u32::MAX as u64),
        64 => Some(u64::MAX),
        _ => None,
    }
}
pub fn logical(
    a: u64,
    b: u64,
    width: u8,
    op: Logic,
    invert: bool,
    shift: LogicalShift,
    amount: u8,
) -> Option<u64> {
    let mask = mask(width)?;
    let b = crate::arm64_logical::evaluate(0, b, width, shift, amount)?;
    let b = if invert { !b } else { b };
    Some(
        (match op {
            Logic::And => a & b,
            Logic::Or => a | b,
            Logic::Xor => a ^ b,
        }) & mask,
    )
}
pub fn logical_flags(result: u64, width: u8) -> Option<Nzcv> {
    let result = result & mask(width)?;
    Some(Nzcv {
        n: result & (1 << (width - 1)) != 0,
        z: result == 0,
        c: false,
        v: false,
    })
}
pub fn selected(
    a: u64,
    b: u64,
    width: u8,
    condition: Condition,
    flags: Nzcv,
    action: SelectAction,
) -> Option<u64> {
    Some(
        (if condition.holds(flags) {
            a
        } else {
            match action {
                SelectAction::Increment => b.wrapping_add(1),
                SelectAction::Invert => !b,
                SelectAction::Negate => b.wrapping_neg(),
            }
        }) & mask(width)?,
    )
}
pub fn extended(value: u64, width: u8, option: u8, amount: u8) -> Option<u64> {
    let mask = mask(width)?;
    if option > 7 || amount > 4 {
        return None;
    }
    let bits = (8u8 << (option & 3)).min(width);
    let low = if bits == 64 {
        value
    } else {
        value & ((1u64 << bits) - 1)
    };
    let value = if option & 4 != 0 && bits < 64 {
        ((low << (64 - bits)) as i64 >> (64 - bits)) as u64
    } else {
        low
    };
    Some(value.wrapping_shl(amount as u32) & mask)
}
pub fn multiply_add(a: u64, b: u64, c: u64, width: u8, subtract: bool) -> Option<u64> {
    let product = a.wrapping_mul(b);
    Some(
        (if subtract {
            c.wrapping_sub(product)
        } else {
            c.wrapping_add(product)
        }) & mask(width)?,
    )
}
pub fn conditional_compare(
    lhs: u64,
    rhs: u64,
    width: u8,
    condition: Condition,
    nzcv: u8,
    subtract: bool,
    incoming: Nzcv,
) -> Option<Nzcv> {
    mask(width)?;
    if nzcv > 15 {
        return None;
    }
    Some(if condition.holds(incoming) {
        arithmetic(lhs, rhs, width, subtract)?.1
    } else {
        Nzcv::from_bits(nzcv)
    })
}
pub fn signed_multiply_high(lhs: u64, rhs: u64) -> u64 {
    let product = (lhs as i64 as i128) * (rhs as i64 as i128);
    (product >> 64) as u64
}
pub fn signed_multiply_add_long(lhs: u64, rhs: u64, addend: u64) -> u64 {
    // Each source is a signed W register, even when Xd aliases Xn/Xm.
    let product = (lhs as i32 as i64) * (rhs as i32 as i64);
    addend.wrapping_add(product as u64)
}
fn read(r: u64, size: u8) -> Varnode {
    if r == 31 {
        Varnode::imm(0, size)
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
pub fn decode(address: u64, word: u32) -> Option<DecodedInstruction> {
    let mut i = DecodedInstruction {
        address,
        bytes: word.to_le_bytes(),
        mnemonic: String::new(),
        raw_args: vec![],
        formatted_args: vec![],
        is_call: false,
        is_ret: false,
        is_conditional_jump: false,
        is_unconditional_jump: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![],
    };
    let size = if word >> 31 != 0 { 8 } else { 4 };
    let width = size * 8;
    let rd = (word & 31) as u64;
    let rn = ((word >> 5) & 31) as u64;
    let rm = ((word >> 16) & 31) as u64;
    let immediate = word & 0x1f800000 == 0x12000000;
    let register = word & 0x1f000000 == 0x0a000000;
    if word & 0x3fe00410 == 0x3a400000 {
        let condition = Condition::from_code(((word >> 12) & 15) as u8)?;
        let nzcv = (word & 15) as u8;
        let subtract = word & (1 << 30) != 0;
        let immediate = word & (1 << 11) != 0;
        let rhs = if immediate {
            Varnode::imm(rm, size)
        } else {
            read(rm, size)
        };
        i.mnemonic = if subtract { "ccmp" } else { "ccmn" }.into();
        i.raw_args = vec![
            name(rn, size),
            if immediate {
                format!("#{rm}")
            } else {
                name(rm, size)
            },
            format!("#{nzcv}"),
            condition.name().into(),
        ];
        i.pcode = vec![PcodeOp::ConditionalCompare {
            lhs: read(rn, size),
            rhs,
            width,
            condition,
            nzcv,
            subtract,
        }];
    } else if word & 0xffe08000 == 0x9b200000 {
        let ra = ((word >> 10) & 31) as u64;
        i.mnemonic = if ra == 31 { "smull" } else { "smaddl" }.into();
        i.raw_args = vec![name(rd, 8), name(rn, 4), name(rm, 4)];
        if ra != 31 {
            i.raw_args.push(name(ra, 8));
        }
        i.pcode = vec![PcodeOp::SignedMultiplyAddLong {
            dst: if rd == 31 {
                None
            } else {
                Some(Varnode::reg(rd, 8))
            },
            lhs: read(rn, 4),
            rhs: read(rm, 4),
            addend: read(ra, 8),
        }];
    } else if word & 0xffe0fc00 == 0x9b407c00 {
        i.mnemonic = "smulh".into();
        i.raw_args = vec![name(rd, 8), name(rn, 8), name(rm, 8)];
        i.pcode = vec![PcodeOp::SignedMultiplyHigh {
            dst: if rd == 31 {
                None
            } else {
                Some(Varnode::reg(rd, 8))
            },
            lhs: read(rn, 8),
            rhs: read(rm, 8),
        }];
    } else if immediate || register {
        let opc = (word >> 29) & 3;
        let invert = register && word & (1 << 21) != 0;
        if opc == 1 && !invert {
            return None;
        } // Existing ORR IR/API stays unchanged.
        let flags = opc == 3;
        let op = match opc {
            0 | 3 => Logic::And,
            1 => Logic::Or,
            _ => Logic::Xor,
        };
        let (rhs, shift, amount, arg) = if immediate {
            let value = immediate_mask(
                width,
                ((word >> 22) & 1) as u8,
                ((word >> 16) & 63) as u8,
                ((word >> 10) & 63) as u8,
            )?;
            (
                Varnode::imm(value, size),
                LogicalShift::Lsl,
                0,
                format!("#0x{value:x}"),
            )
        } else {
            let amount = ((word >> 10) & 63) as u8;
            if amount >= width {
                return None;
            }
            let shift = match (word >> 22) & 3 {
                0 => LogicalShift::Lsl,
                1 => LogicalShift::Lsr,
                2 => LogicalShift::Asr,
                _ => LogicalShift::Ror,
            };
            (
                read(rm, size),
                shift,
                amount,
                format!("{}, {:?} #{amount}", name(rm, size), shift).to_lowercase(),
            )
        };
        let dst = if rd == 31 && (!immediate || flags) {
            None
        } else {
            Some(Varnode::reg(rd, size))
        };
        i.mnemonic = match (op, invert, flags) {
            (Logic::And, false, false) => "and",
            (Logic::And, false, true) => "ands",
            (Logic::And, true, false) => "bic",
            (Logic::And, true, true) => "bics",
            (Logic::Or, true, _) => "orn",
            (Logic::Xor, false, _) => "eor",
            (Logic::Xor, true, _) => "eon",
            _ => return None,
        }
        .into();
        i.raw_args = vec![
            dst.map(|r| r.display_arm64())
                .unwrap_or_else(|| name(rd, size)),
            name(rn, size),
            arg,
        ];
        i.pcode = vec![PcodeOp::Logical {
            dst,
            lhs: read(rn, size),
            rhs,
            op,
            invert,
            shift,
            amount,
            width,
            set_flags: flags,
        }];
    } else if word & 0x3fe00800 == 0x1a800000 {
        let action = match ((word >> 29) & 2) | ((word >> 10) & 1) {
            0 => return None,
            1 => SelectAction::Increment,
            2 => SelectAction::Invert,
            _ => SelectAction::Negate,
        };
        let condition = Condition::from_code(((word >> 12) & 15) as u8)?;
        i.mnemonic = match action {
            SelectAction::Increment => "csinc",
            SelectAction::Invert => "csinv",
            SelectAction::Negate => "csneg",
        }
        .into();
        i.raw_args = vec![
            name(rd, size),
            name(rn, size),
            name(rm, size),
            condition.name().into(),
        ];
        i.pcode = vec![PcodeOp::ConditionalTransform {
            dst: if rd == 31 {
                None
            } else {
                Some(Varnode::reg(rd, size))
            },
            if_true: read(rn, size),
            if_false: read(rm, size),
            condition,
            action,
            width,
        }];
    } else if word & 0x7fe00000 == 0x1b000000 {
        let ra = ((word >> 10) & 31) as u64;
        let subtract = word & (1 << 15) != 0;
        i.mnemonic = if subtract { "msub" } else { "madd" }.into();
        i.raw_args = vec![
            name(rd, size),
            name(rn, size),
            name(rm, size),
            name(ra, size),
        ];
        i.pcode = vec![PcodeOp::MultiplyAdd {
            dst: if rd == 31 {
                None
            } else {
                Some(Varnode::reg(rd, size))
            },
            lhs: read(rn, size),
            rhs: read(rm, size),
            addend: read(ra, size),
            width,
            subtract,
        }];
    } else {
        let extension = if word & 0x1fe00000 == 0x0b200000 {
            Some(((word >> 13) & 7) as u8)
        } else {
            None
        };
        let shifted_sub = word & 0x7f200000 == 0x4b000000;
        if extension.is_none() && !shifted_sub {
            return None;
        }
        let subtract = word & (1 << 30) != 0;
        let flags = word & (1 << 29) != 0;
        let amount = if extension.is_some() {
            ((word >> 10) & 7) as u8
        } else {
            ((word >> 10) & 63) as u8
        };
        let shift = if extension.is_some() {
            if amount > 4 {
                return None;
            }
            Shift::Lsl
        } else {
            if amount >= width {
                return None;
            }
            match (word >> 22) & 3 {
                0 => Shift::Lsl,
                1 => Shift::Lsr,
                2 => Shift::Asr,
                _ => return None,
            }
        };
        let lhs = if extension.is_some() {
            Varnode::reg(rn, size)
        } else {
            read(rn, size)
        };
        let rhs_size = if size == 8 && extension.is_some_and(|e| e & 3 == 3) {
            8
        } else if extension.is_some() {
            4
        } else {
            size
        };
        let rhs = read(rm, rhs_size);
        let dst = if rd == 31 && (extension.is_none() || flags) {
            None
        } else {
            Some(Varnode::reg(rd, size))
        };
        i.mnemonic = match (subtract, flags, dst.is_none()) {
            (true, true, true) => "cmp",
            (false, true, true) => "cmn",
            (true, true, false) => "subs",
            (false, true, false) => "adds",
            (true, false, _) => "sub",
            _ => "add",
        }
        .into();
        if !(flags && dst.is_none()) {
            i.raw_args.push(
                dst.map(|r| r.display_arm64())
                    .unwrap_or_else(|| name(rd, size)),
            );
        }
        i.raw_args.push(if extension.is_some() {
            lhs.display_arm64()
        } else {
            name(rn, size)
        });
        let modifier = extension
            .map(|e| {
                [
                    "uxtb", "uxth", "uxtw", "uxtx", "sxtb", "sxth", "sxtw", "sxtx",
                ][e as usize]
                    .to_string()
            })
            .unwrap_or_else(|| format!("{shift:?}").to_lowercase());
        i.raw_args
            .push(format!("{}, {modifier} #{amount}", name(rm, rhs_size)));
        i.pcode = vec![PcodeOp::RegisterArithmetic {
            dst,
            lhs,
            rhs,
            extension,
            shift,
            amount,
            width,
            subtract,
            set_flags: flags,
        }];
    }
    i.formatted_args = i.raw_args.clone();
    Some(i)
}
/// Semantic primitive used by the concrete validation runner.
#[allow(clippy::too_many_arguments)]
pub fn register_arithmetic(
    a: u64,
    b: u64,
    width: u8,
    extension: Option<u8>,
    shift: Shift,
    amount: u8,
    subtract: bool,
    set_flags: bool,
) -> Option<(u64, Option<Nzcv>)> {
    let b = if let Some(option) = extension {
        extended(b, width, option, amount)?
    } else {
        shifted(b, width, shift, amount)?
    };
    let (value, flags) = arithmetic(a, b, width, subtract)?;
    Some((value, if set_flags { Some(flags) } else { None }))
}
