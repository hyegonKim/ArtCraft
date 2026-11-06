//! Atomic RMW and explicit local/global exclusive-monitor operations. A failed
//! exclusive store does not write memory; every attempt consumes the reservation.
use crate::pcode_sleigh::{DecodedInstruction, PcodeOp, Varnode};
pub fn add_value(old: u64, addend: u64, width: u8) -> Option<(u64, u64)> {
    let mask = match width {
        32 => u32::MAX as u64,
        64 => u64::MAX,
        _ => return None,
    };
    Some((old & mask, old.wrapping_add(addend) & mask))
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
        is_conditional_jump: false,
        is_unconditional_jump: false,
        is_call: false,
        is_ret: false,
        branch_target: None,
        call_ref: None,
        pcode: vec![],
    };
    let rt = (word & 31) as u64;
    let rn = ((word >> 5) & 31) as u64;
    let rs = ((word >> 16) & 31) as u64;
    let size = if word & (1 << 30) != 0 { 8 } else { 4 };
    let addr = Varnode::reg(rn, 8);
    if word & 0x3f20fc00 == 0x38200000 && word >> 30 >= 2 && rt != 31 {
        let acquire = word & (1 << 23) != 0;
        let release = word & (1 << 22) != 0;
        i.mnemonic = format!(
            "ldadd{}",
            match (acquire, release) {
                (true, true) => "al",
                (true, false) => "a",
                (false, true) => "l",
                _ => "",
            }
        );
        i.raw_args = vec![
            name(rs, size),
            name(rt, size),
            format!("[{}]", addr.display_arm64()),
        ];
        i.pcode = vec![PcodeOp::AtomicAdd {
            dst: Varnode::reg(rt, size),
            src: read(rs, size),
            addr,
            acquire,
            release,
        }];
    } else if word & 0xbffffc00 == 0x885ffc00 && rt != 31 {
        i.mnemonic = "ldaxr".into();
        i.raw_args = vec![name(rt, size), format!("[{}]", addr.display_arm64())];
        i.pcode = vec![PcodeOp::ExclusiveLoadAcquire {
            dst: Varnode::reg(rt, size),
            addr,
        }];
    } else if word & 0xbfe0fc00 == 0x8800fc00 && rs != 31 && rs != rt && (rn == 31 || rs != rn) {
        i.mnemonic = "stlxr".into();
        i.raw_args = vec![
            name(rs, 4),
            name(rt, size),
            format!("[{}]", addr.display_arm64()),
        ];
        i.pcode = vec![PcodeOp::ExclusiveStoreRelease {
            status: Varnode::reg(rs, 4),
            src: read(rt, size),
            addr,
        }];
    } else if word & 0xfffff0ff == 0xd503305f {
        i.mnemonic = "clrex".into();
        i.raw_args = vec![format!("#{}", (word >> 8) & 15)];
        i.pcode = vec![PcodeOp::ClearExclusive];
    } else {
        return None;
    }
    i.formatted_args = i.raw_args.clone();
    Some(i)
}
