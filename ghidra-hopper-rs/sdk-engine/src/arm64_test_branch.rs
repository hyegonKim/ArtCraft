//! TBZ/TBNZ test a register bit directly. Neither reads nor writes NZCV.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, Varnode};

pub fn taken(value: u64, bit: u8, nonzero: bool) -> Option<bool> {
    if bit >= 64 {
        return None;
    }
    Some(((value >> bit) & 1 != 0) == nonzero)
}

pub fn decode(address: u64, word: u32) -> Option<DecodedInstruction> {
    if word & 0x7e000000 != 0x36000000 {
        return None;
    }
    let nonzero = word & (1 << 24) != 0;
    let bit = (((word >> 31) << 5) | ((word >> 19) & 31)) as u8;
    let size = if bit < 32 { 4 } else { 8 };
    let rt = (word & 31) as u64;
    let imm = ((word >> 5) & 0x3fff) as i64;
    let displacement = ((imm << 50) >> 50) * 4;
    let target = address.wrapping_add(displacement as u64);
    let value = if rt == 31 {
        Varnode::imm(0, size)
    } else {
        Varnode::reg(rt, size)
    };
    let reg_name = if rt == 31 {
        if size == 8 { "xzr" } else { "wzr" }.to_string()
    } else {
        value.display_arm64()
    };
    let args = vec![reg_name, format!("#{bit}"), format!("0x{target:x}")];
    Some(DecodedInstruction {
        address,
        bytes: word.to_le_bytes(),
        mnemonic: if nonzero { "tbnz" } else { "tbz" }.into(),
        raw_args: args.clone(),
        formatted_args: args,
        is_conditional_jump: true,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: Some(target),
        call_ref: None,
        pcode: vec![PcodeOp::TestBitBranch {
            value,
            bit,
            nonzero,
            target,
        }],
    })
}
