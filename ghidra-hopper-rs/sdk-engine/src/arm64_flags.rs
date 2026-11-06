//! Integer arithmetic (with and without NZCV writes) and A64 conditions. Branches consume the
//! architectural flag state, not invented general-purpose registers.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, Varnode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nzcv {
    pub n: bool,
    pub z: bool,
    pub c: bool,
    pub v: bool,
}
impl Nzcv {
    pub fn from_bits(bits: u8) -> Self {
        Self {
            n: bits & 8 != 0,
            z: bits & 4 != 0,
            c: bits & 2 != 0,
            v: bits & 1 != 0,
        }
    }
    pub fn bits(self) -> u8 {
        (self.n as u8) * 8 + (self.z as u8) * 4 + (self.c as u8) * 2 + self.v as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition(u8);
impl Condition {
    pub fn from_code(code: u8) -> Option<Self> {
        if code < 16 {
            Some(Self(code))
        } else {
            None
        }
    }
    pub fn code(self) -> u8 {
        self.0
    }
    pub fn name(self) -> &'static str {
        [
            "eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le",
            "al", "nv",
        ][self.0 as usize]
    }
    pub fn expression(self) -> &'static str {
        [
            "nzcv.Z",
            "!nzcv.Z",
            "nzcv.C",
            "!nzcv.C",
            "nzcv.N",
            "!nzcv.N",
            "nzcv.V",
            "!nzcv.V",
            "nzcv.C && !nzcv.Z",
            "!nzcv.C || nzcv.Z",
            "nzcv.N == nzcv.V",
            "nzcv.N != nzcv.V",
            "!nzcv.Z && (nzcv.N == nzcv.V)",
            "nzcv.Z || (nzcv.N != nzcv.V)",
            "true",
            "true",
        ][self.0 as usize]
    }
    pub fn holds(self, f: Nzcv) -> bool {
        match self.0 {
            0 => f.z,
            1 => !f.z,
            2 => f.c,
            3 => !f.c,
            4 => f.n,
            5 => !f.n,
            6 => f.v,
            7 => !f.v,
            8 => f.c && !f.z,
            9 => !f.c || f.z,
            10 => f.n == f.v,
            11 => f.n != f.v,
            12 => !f.z && (f.n == f.v),
            13 => f.z || (f.n != f.v),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    Lsl,
    Lsr,
    Asr,
}
pub fn shifted(value: u64, width: u8, shift: Shift, amount: u8) -> Option<u64> {
    if !matches!(width, 32 | 64) || amount >= width {
        return None;
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        u32::MAX as u64
    };
    let value = value & mask;
    Some(
        match shift {
            Shift::Lsl => value << amount,
            Shift::Lsr => value >> amount,
            Shift::Asr => {
                if width == 64 {
                    ((value as i64) >> amount) as u64
                } else {
                    ((value as i32) >> amount) as u32 as u64
                }
            }
        } & mask,
    )
}

pub fn add_shifted(lhs: u64, rhs: u64, width: u8, shift: Shift, amount: u8) -> Option<u64> {
    let operand = shifted(rhs, width, shift, amount)?;
    let mask = if width == 64 {
        u64::MAX
    } else {
        u32::MAX as u64
    };
    Some(lhs.wrapping_add(operand) & mask)
}

pub fn arithmetic(lhs: u64, rhs: u64, width: u8, subtract: bool) -> Option<(u64, Nzcv)> {
    if !matches!(width, 32 | 64) {
        return None;
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        u32::MAX as u64
    };
    let sign = 1u64 << (width - 1);
    let a = lhs & mask;
    let b = rhs & mask;
    let result = if subtract {
        a.wrapping_sub(b)
    } else {
        a.wrapping_add(b)
    } & mask;
    let c = if subtract {
        a >= b
    } else {
        a as u128 + b as u128 > mask as u128
    };
    let v = if subtract {
        (a ^ b) & (a ^ result) & sign != 0
    } else {
        (!(a ^ b)) & (a ^ result) & sign != 0
    };
    Some((
        result,
        Nzcv {
            n: result & sign != 0,
            z: result == 0,
            c,
            v,
        },
    ))
}

fn zero_reg(index: u64, size: u8) -> Varnode {
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
    let mut insn = DecodedInstruction {
        address,
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
    if word & 0xff000010 == 0x54000000 {
        let condition = Condition((word & 15) as u8);
        let imm = ((word >> 5) & 0x7ffff) as i64;
        let target = address.wrapping_add((((imm << 45) >> 45) * 4) as u64);
        insn.mnemonic = format!("b.{}", condition.name());
        insn.raw_args = vec![format!("0x{target:x}")];
        insn.is_conditional_jump = condition.code() < 14;
        insn.is_unconditional_jump = condition.code() >= 14;
        insn.branch_target = Some(target);
        insn.pcode = vec![PcodeOp::FlagBranch { condition, target }];
    } else {
        let immediate = word & 0x1f800000 == 0x11000000;
        let register = word & 0x1f200000 == 0x0b000000;
        let set_flags = word & (1 << 29) != 0;
        let subtract = word & (1 << 30) != 0;
        // Non-S shifted ADD is supported here; immediate ADD stays in the
        // existing decoder, and non-S shifted SUB remains unsupported.
        if !(immediate || register) || (!set_flags && (!register || subtract)) {
            return None;
        }
        let size = if word >> 31 != 0 { 8 } else { 4 };
        let rd = (word & 31) as u64;
        let rn = ((word >> 5) & 31) as u64;
        let (lhs, rhs, shift, amount, rhs_args) = if immediate {
            let value =
                (((word >> 10) & 4095) as u64) << if word & (1 << 22) != 0 { 12 } else { 0 };
            (
                Varnode::reg(rn, size),
                Varnode::imm(value, size),
                Shift::Lsl,
                0,
                vec![format!("#0x{value:x}")],
            )
        } else {
            let shift = match (word >> 22) & 3 {
                0 => Shift::Lsl,
                1 => Shift::Lsr,
                2 => Shift::Asr,
                _ => return None,
            };
            let amount = ((word >> 10) & 63) as u8;
            if amount >= size * 8 {
                return None;
            }
            let rm = ((word >> 16) & 31) as u64;
            let mut args = vec![reg_name(rm, size)];
            if amount != 0 {
                args.push(format!(
                    "{} #{amount}",
                    match shift {
                        Shift::Lsl => "lsl",
                        Shift::Lsr => "lsr",
                        Shift::Asr => "asr",
                    }
                ));
            }
            (zero_reg(rn, size), zero_reg(rm, size), shift, amount, args)
        };
        insn.mnemonic = if !set_flags {
            "add"
        } else {
            match (rd == 31, subtract) {
                (true, true) => "cmp",
                (true, false) => "cmn",
                (false, true) => "subs",
                _ => "adds",
            }
        }
        .into();
        if rd != 31 || !set_flags {
            insn.raw_args.push(reg_name(rd, size));
        }
        insn.raw_args.push(if immediate {
            lhs.display_arm64()
        } else {
            reg_name(rn, size)
        });
        insn.raw_args.extend(rhs_args);
        let dst = if rd == 31 {
            None
        } else {
            Some(Varnode::reg(rd, size))
        };
        insn.pcode = if !set_flags {
            vec![PcodeOp::ShiftedAdd {
                dst,
                lhs,
                rhs,
                shift,
                amount,
            }]
        } else {
            vec![PcodeOp::FlagArithmetic {
                dst,
                lhs,
                rhs,
                shift,
                amount,
                subtract,
            }]
        };
    }
    insn.formatted_args = insn.raw_args.clone();
    Some(insn)
}
