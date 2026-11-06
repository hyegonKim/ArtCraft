//! Integer register-offset transfers. Address inputs are captured before loads
//! overwrite registers; extension precedes scaling. No base writeback occurs.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexExtend {
    Uxtw,
    Uxtx,
    Sxtw,
    Sxtx,
}

pub fn address(base: u64, index: u64, extend: IndexExtend, shift: u8) -> Option<u64> {
    if shift > 1 {
        return None;
    }
    offset_address(base, index, extend, shift)
}

pub fn offset_address(base: u64, index: u64, extend: IndexExtend, shift: u8) -> Option<u64> {
    // Q register-offset loads scale by 16; integer transfers still decode 0..3.
    if shift > 4 {
        return None;
    }
    let offset = match extend {
        IndexExtend::Uxtw => index as u32 as u64,
        IndexExtend::Uxtx => index,
        IndexExtend::Sxtw => index as u32 as i32 as i64 as u64,
        IndexExtend::Sxtx => index as i64 as u64,
    };
    Some(base.wrapping_add(offset.wrapping_shl(shift as u32)))
}

fn temp(offset: u64, size: u8) -> Varnode {
    Varnode {
        space: SpaceKind::Unique,
        offset,
        size,
    }
}
pub fn decode(pc: u64, word: u32) -> Option<DecodedInstruction> {
    if word & 0x3f200c00 != 0x38200800 {
        return None;
    }
    let memory_size = 1u8 << ((word >> 30) & 3);
    let opc = (word >> 22) & 3;
    let signed_word = opc == 2 && memory_size == 4;
    if opc > 1 && !signed_word {
        return None;
    }
    let option = (word >> 13) & 7;
    let (extend, index_size, extend_name) = match option {
        2 => (IndexExtend::Uxtw, 4, "uxtw"),
        3 => (IndexExtend::Uxtx, 8, "lsl"),
        6 => (IndexExtend::Sxtw, 4, "sxtw"),
        7 => (IndexExtend::Sxtx, 8, "sxtx"),
        _ => return None,
    };
    let shift = if word & (1 << 12) != 0 {
        ((word >> 30) & 3) as u8
    } else {
        0
    };
    let rn = ((word >> 5) & 31) as u64;
    let rm = ((word >> 16) & 31) as u64;
    let rt = (word & 31) as u64;
    let base = Varnode::reg(rn, 8);
    let index = if rm == 31 {
        Varnode::imm(0, index_size)
    } else {
        Varnode::reg(rm, index_size)
    };
    let register_size = if memory_size == 8 || signed_word {
        8
    } else {
        4
    };
    let dst = if rt == 31 {
        temp(2, register_size)
    } else {
        Varnode::reg(rt, register_size)
    };
    let index_name = if rm == 31 {
        if index_size == 4 { "wzr" } else { "xzr" }.into()
    } else {
        index.display_arm64()
    };
    let destination = if rt == 31 {
        if register_size == 8 { "xzr" } else { "wzr" }.into()
    } else {
        dst.display_arm64()
    };
    let extension = if option == 3 && shift == 0 {
        String::new()
    } else {
        format!(", {extend_name} #{shift}")
    };
    let args = vec![
        destination,
        format!("[{}, {}{}]", base.display_arm64(), index_name, extension),
    ];
    let mnemonic = match (opc, memory_size) {
        (0, 1) => "strb",
        (0, 2) => "strh",
        (0, _) => "str",
        (1, 1) => "ldrb",
        (1, 2) => "ldrh",
        (1, _) => "ldr",
        _ => "ldrsw",
    };
    let mut pcode = vec![PcodeOp::RegisterOffset {
        dst: temp(0, 8),
        base,
        index,
        extend,
        shift,
    }];
    if opc == 0 {
        pcode.push(PcodeOp::Store {
            addr: temp(0, 8),
            src: if rt == 31 {
                Varnode::imm(0, memory_size)
            } else {
                Varnode::reg(rt, memory_size)
            },
        });
    } else {
        pcode.push(PcodeOp::Load {
            dst: temp(1, memory_size),
            addr: temp(0, 8),
        });
        pcode.push(if signed_word {
            PcodeOp::SignExtend {
                dst,
                src: temp(1, 4),
            }
        } else {
            PcodeOp::ZeroExtend {
                dst,
                src: temp(1, memory_size),
            }
        });
    }
    Some(DecodedInstruction {
        address: pc,
        bytes: word.to_le_bytes(),
        mnemonic: mnemonic.into(),
        raw_args: args.clone(),
        formatted_args: args,
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode,
    })
}
