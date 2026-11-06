//! ADR/ADRP form addresses. They do not load memory, branch, or modify NZCV.
use crate::{
    pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode},
    segment::Segment,
};

pub fn evaluate(pc: u64, encoded_imm21: u32, page: bool) -> Option<u64> {
    if encoded_imm21 >= 1 << 21 {
        return None;
    }
    let displacement = ((encoded_imm21 as i64) << 43) >> 43;
    let base = if page { pc & !0xfff } else { pc };
    let displacement = if page {
        displacement << 12
    } else {
        displacement
    };
    Some(base.wrapping_add(displacement as u64))
}

pub fn decode(address: u64, word: u32, seg: &Segment) -> Option<DecodedInstruction> {
    let page = match word & 0x9f000000 {
        0x10000000 => false,
        0x90000000 => true,
        _ => return None,
    };
    let immediate = (((word >> 5) & 0x7ffff) << 2) | ((word >> 29) & 3);
    let target = evaluate(address, immediate, page)?;
    let rd = (word & 31) as u64;
    // Rd=31 discards to XZR in both encodings. It never writes SP.
    let dst = if rd == 31 {
        Varnode {
            space: SpaceKind::Unique,
            offset: 5,
            size: 8,
        }
    } else {
        Varnode::reg(rd, 8)
    };
    let name = if rd == 31 {
        "xzr".into()
    } else {
        dst.display_arm64()
    };
    let formatted = seg
        .operand_formats
        .get(&(address, 1))
        .copied()
        .unwrap_or_default()
        .format_imm(target);
    Some(DecodedInstruction {
        address,
        bytes: word.to_le_bytes(),
        mnemonic: if page { "adrp" } else { "adr" }.into(),
        raw_args: vec![name.clone(), format!("0x{target:x}")],
        formatted_args: vec![name, formatted],
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![PcodeOp::Copy {
            dst,
            src: Varnode::imm(target, 8),
        }],
    })
}
