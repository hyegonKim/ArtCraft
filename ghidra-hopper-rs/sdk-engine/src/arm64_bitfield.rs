use crate::pcode_sleigh::{BitfieldKind, DecodedInstruction, PcodeOp, SpaceKind, Varnode};

/// ARM bitfield mask semantics, without shifts by the register width.
pub fn evaluate(
    kind: BitfieldKind,
    width: u8,
    immr: u8,
    imms: u8,
    source: u64,
    old: u64,
) -> Option<u64> {
    if !matches!(width, 32 | 64) || immr >= width || imms >= width {
        return None;
    }
    let ones = |n: u8| if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
    let mask = ones(width);
    let source = source & mask;
    let rotate = |v: u64| {
        if immr == 0 {
            v & mask
        } else {
            ((v >> immr) | (v << (width - immr))) & mask
        }
    };
    let wmask = rotate(ones(imms + 1));
    let tmask = ones((imms.wrapping_sub(immr) & (width - 1)) + 1);
    let bot = rotate(source) & wmask;
    Some(
        (match kind {
            BitfieldKind::Unsigned => bot & tmask,
            BitfieldKind::Signed => {
                let top = if source & (1u64 << imms) != 0 {
                    mask
                } else {
                    0
                };
                (top & !tmask) | (bot & tmask)
            }
            BitfieldKind::Insert => {
                let bot = (old & !wmask) | bot;
                (old & !tmask) | (bot & tmask)
            }
        }) & mask,
    )
}

pub fn decode(address: u64, word: u32) -> Option<DecodedInstruction> {
    if word & 0x1f800000 != 0x13000000 {
        return None;
    }
    let sf = word >> 31;
    let n = (word >> 22) & 1;
    let width = if sf == 1 { 64 } else { 32 };
    let immr = ((word >> 16) & 63) as u8;
    let imms = ((word >> 10) & 63) as u8;
    if n != sf || immr >= width || imms >= width {
        return None;
    }
    let (kind, mnemonic) = match (word >> 29) & 3 {
        0 => (BitfieldKind::Signed, "sbfm"),
        1 => (BitfieldKind::Insert, "bfm"),
        2 => (BitfieldKind::Unsigned, "ubfm"),
        _ => return None,
    };
    let rd = (word & 31) as u64;
    let rn = ((word >> 5) & 31) as u64;
    let size = width / 8;
    let dst = if rd == 31 {
        Varnode {
            space: SpaceKind::Unique,
            offset: 3,
            size,
        }
    } else {
        Varnode::reg(rd, size)
    };
    let src = if rn == 31 {
        Varnode::imm(0, size)
    } else {
        Varnode::reg(rn, size)
    };
    let name = |reg: u64| {
        if reg == 31 {
            if size == 8 { "xzr" } else { "wzr" }.into()
        } else {
            Varnode::reg(reg, size).display_arm64()
        }
    };
    let args = vec![name(rd), name(rn), format!("#{immr}"), format!("#{imms}")];
    Some(DecodedInstruction {
        address,
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
        pcode: vec![PcodeOp::Bitfield {
            dst,
            src,
            old_dst: if rd == 31 { Varnode::imm(0, size) } else { dst },
            kind,
            immr,
            imms,
        }],
    })
}
