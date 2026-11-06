//! Conservative recovery of CMP/B.HI guarded unsigned-halfword relative tables.
//! No address, register number, or table contents are specific to an input binary.
use crate::{
    arm64_flags::Shift,
    arm64_indexed::IndexExtend,
    pcode_sleigh::{DecodedInstruction, PcodeOp, SpaceKind, Varnode},
    segment::Segment,
};
use std::collections::{BTreeMap, BTreeSet};

type Instructions = BTreeMap<u64, DecodedInstruction>;
type Predecessors = BTreeMap<u64, BTreeSet<u64>>;

pub(crate) fn successors(i: &DecodedInstruction) -> Vec<u64> {
    if i.is_ret || matches!(i.pcode.first(), Some(PcodeOp::Unsupported { .. })) {
        return vec![];
    }
    if let Some(PcodeOp::TableBranch { targets, .. }) = i.pcode.first() {
        return targets
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    if let Some(PcodeOp::BranchInd { switch_targets, .. }) = i.pcode.first() {
        return switch_targets
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    let mut out = Vec::new();
    if !i.is_call {
        out.extend(i.branch_target);
    }
    if !i.is_unconditional_jump
        && !i
            .pcode
            .iter()
            .any(|op| matches!(op, PcodeOp::UnverifiedContinuation { .. }))
    {
        out.extend(i.address.checked_add(4));
    }
    out
}

fn predecessors(decoded: &Instructions) -> Predecessors {
    let mut result: Predecessors = BTreeMap::new();
    for i in decoded.values() {
        for target in successors(i) {
            result.entry(target).or_default().insert(i.address);
        }
    }
    result
}

fn destination(op: &PcodeOp) -> Option<Varnode> {
    op.value_destination()
}

struct Proof<'a> {
    decoded: &'a Instructions,
    preds: Predecessors,
    entry: u64,
}
impl Proof<'_> {
    /// Every reaching definition must be known. Loops are traversed once; an
    /// entry path without a definition, or an ABI-clobbering call, rejects it.
    fn definitions(&self, at: u64, reg: u64) -> Option<BTreeSet<(u64, usize)>> {
        if at == self.entry {
            return None;
        }
        let mut pending = self.preds.get(&at)?.clone();
        let mut visited = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        while let Some(pc) = pending.pop_first() {
            if !visited.insert(pc) {
                continue;
            }
            let i = self.decoded.get(&pc)?;
            // AAPCS64 x19-x28 are callee-saved; caller-saved values are unknown.
            if i.is_call && !(19..=28).contains(&reg) {
                return None;
            }
            if let Some((n, _)) = i.pcode.iter().enumerate().rev().find(|(_, op)| {
                destination(op)
                    .is_some_and(|dst| dst.space == SpaceKind::Register && dst.offset == reg)
            }) {
                definitions.insert((pc, n));
                continue;
            }
            if pc == self.entry {
                return None;
            }
            pending.extend(self.preds.get(&pc)?);
        }
        (!definitions.is_empty()).then_some(definitions)
    }

    fn constant(&self, at: u64, value: Varnode, depth: usize) -> Option<u64> {
        if value.space == SpaceKind::Constant {
            return Some(value.offset);
        }
        if value.space != SpaceKind::Register || depth > 16 {
            return None;
        }
        let mut values = BTreeSet::new();
        for (pc, n) in self.definitions(at, value.offset)? {
            let op = &self.decoded.get(&pc)?.pcode[n];
            let raw = match op {
                PcodeOp::Copy { src, .. } => self.constant(pc, *src, depth + 1)?,
                PcodeOp::IntAdd { lhs, rhs, .. } => self
                    .constant(pc, *lhs, depth + 1)?
                    .checked_add(self.constant(pc, *rhs, depth + 1)?)?,
                _ => return None,
            };
            let dst = destination(op)?;
            values.insert(if dst.size == 4 {
                raw & u32::MAX as u64
            } else {
                raw
            });
        }
        if values.len() != 1 {
            return None;
        }
        let result = *values.first()?;
        Some(if value.size == 4 {
            result & u32::MAX as u64
        } else {
            result
        })
    }

    fn recover(&self, pc: u64, seg: &Segment) -> Option<PcodeOp> {
        let branch = self.decoded.get(&pc)?;
        let target_reg = match branch.pcode.as_slice() {
            [PcodeOp::BranchInd {
                target_reg,
                switch_targets,
            }] if switch_targets.is_empty() => *target_reg,
            [PcodeOp::TableBranch { target_reg, .. }] => *target_reg,
            _ => return None,
        };
        let add_pc = pc.checked_sub(4)?;
        let load_pc = pc.checked_sub(8)?;
        let adr_pc = pc.checked_sub(12)?;
        let [PcodeOp::ShiftedAdd {
            dst: Some(dst),
            lhs,
            rhs,
            shift: Shift::Lsl,
            amount: 2,
        }] = self.decoded.get(&add_pc)?.pcode.as_slice()
        else {
            return None;
        };
        if *dst != target_reg
            || *lhs != target_reg
            || rhs.space != SpaceKind::Register
            || rhs.size != 8
        {
            return None;
        }
        let [PcodeOp::RegisterOffset {
            dst: addr,
            base,
            index,
            extend,
            shift: 1,
        }, PcodeOp::Load {
            dst: half,
            addr: load_addr,
        }, PcodeOp::ZeroExtend { dst: loaded, src }] = self.decoded.get(&load_pc)?.pcode.as_slice()
        else {
            return None;
        };
        if addr != load_addr
            || half != src
            || half.size != 2
            || loaded.space != SpaceKind::Register
            || loaded.size != 4
            || loaded.offset != rhs.offset
        {
            return None;
        }
        if index.space != SpaceKind::Register
            || index.offset == 31
            || !matches!(extend, IndexExtend::Uxtw | IndexExtend::Uxtx)
        {
            return None;
        }
        if index.offset == target_reg.offset
            || index.offset == rhs.offset
            || base.offset == target_reg.offset
        {
            return None;
        }
        let [PcodeOp::Copy {
            dst: adr_dst,
            src: origin,
        }] = self.decoded.get(&adr_pc)?.pcode.as_slice()
        else {
            return None;
        };
        if *adr_dst != target_reg
            || origin.space != SpaceKind::Constant
            || self.decoded.get(&adr_pc)?.mnemonic != "adr"
        {
            return None;
        }

        // Allow up to four ordinary instructions between B.HI and ADR. The
        // guarded dispatch interior must have exactly its linear predecessor.
        let guard_pc = (1..=5).filter_map(|n| adr_pc.checked_sub(4 * n)).find(|a| {
            self.decoded.get(a).is_some_and(|i| matches!(i.pcode.first(), Some(PcodeOp::FlagBranch { condition, .. }) if condition.code() == 8))
        })?;
        let compare_pc = guard_pc.checked_sub(4)?;
        let [PcodeOp::FlagArithmetic {
            dst: None,
            lhs: compared,
            rhs: bound,
            shift: Shift::Lsl,
            amount: 0,
            subtract: true,
        }] = self.decoded.get(&compare_pc)?.pcode.as_slice()
        else {
            return None;
        };
        if compared.space != SpaceKind::Register
            || compared.offset != index.offset
            || bound.space != SpaceKind::Constant
            || bound.offset >= 4096
        {
            return None;
        }
        let default = self.decoded.get(&guard_pc)?.branch_target?;
        if (guard_pc + 4..=pc).contains(&default) {
            return None;
        }
        for address in (compare_pc + 4..=pc).step_by(4) {
            if address == self.entry || self.preds.get(&address)? != &BTreeSet::from([address - 4])
            {
                return None;
            }
            let i = self.decoded.get(&address)?;
            if address > guard_pc
                && address < pc
                && (i.is_call || i.is_ret || i.is_conditional_jump || i.is_unconditional_jump)
            {
                return None;
            }
            if address > guard_pc
                && address < load_pc
                && i.pcode.iter().any(|op| {
                    destination(op)
                        .is_some_and(|d| d.space == SpaceKind::Register && d.offset == index.offset)
                })
            {
                return None;
            }
        }
        // CMP Wn bounds only low bits. A 64-bit table index is safe only when
        // every reaching definition zero-extended a W-register write.
        if compared.size == 4 && *extend == IndexExtend::Uxtx {
            for (address, n) in self.definitions(compare_pc, index.offset)? {
                if destination(&self.decoded.get(&address)?.pcode[n])?.size != 4 {
                    return None;
                }
            }
        }
        let table_address = self.constant(load_pc, *base, 0)?;
        let entries = usize::try_from(bound.offset.checked_add(1)?).ok()?;
        let length = entries.checked_mul(2)?;
        let table_end = table_address.checked_add(length as u64)?;
        // Only immutable, file-backed Mach-O regular data is accepted here.
        if seg.permissions != Some(5)
            || !seg.sections.iter().any(|s| {
                s.name == "__const"
                    && s.flags & 0xff == 0
                    && s.flags & 0x80000400 == 0
                    && table_address >= s.start_address
                    && s.start_address
                        .checked_add(s.length)
                        .is_some_and(|end| table_end <= end)
            })
        {
            return None;
        }
        let bytes = seg.read_bytes(table_address, length)?;
        let mut targets = Vec::with_capacity(entries);
        for half in bytes.chunks_exact(2) {
            let offset = u16::from_le_bytes([half[0], half[1]]) as u64;
            let target = origin.offset.checked_add(offset.checked_mul(4)?)?;
            if !target.is_multiple_of(4)
                || seg.read_bytes(target, 4).is_none()
                || !seg.sections.iter().any(|s| {
                    s.flags & 0x80000400 != 0
                        && target >= s.start_address
                        && s.start_address.checked_add(s.length).is_some_and(|end| {
                            target.checked_add(4).is_some_and(|last| last <= end)
                        })
                })
            {
                return None;
            }
            // A candidate edge must not bypass its own range check or overwrite
            // an instruction in the proven dispatch sequence.
            if (compare_pc + 4..=pc).contains(&target) {
                return None;
            }
            targets.push(target);
        }
        Some(PcodeOp::TableBranch {
            target_reg,
            index: *compared,
            table_address,
            offset_base: origin.offset,
            targets,
        })
    }
}

pub(crate) fn recover(decoded: &Instructions, entry: u64, seg: &Segment) -> Vec<(u64, PcodeOp)> {
    let proof = Proof {
        decoded,
        preds: predecessors(decoded),
        entry,
    };
    decoded
        .keys()
        .filter_map(|&pc| proof.recover(pc, seg).map(|op| (pc, op)))
        .collect()
}

/// Late-discovered back edges can invalidate reaching definitions. Never keep
/// a table proven against an older graph, nor instructions reachable only by it.
pub(crate) fn validate(decoded: &mut Instructions, entry: u64, seg: &Segment) {
    loop {
        let valid: BTreeMap<_, _> = recover(decoded, entry, seg).into_iter().collect();
        let mut changed = false;
        for (&pc, i) in decoded.iter_mut() {
            if let Some(PcodeOp::TableBranch { target_reg, .. }) = i.pcode.first() {
                if valid.get(&pc) != i.pcode.first() {
                    i.pcode = vec![PcodeOp::BranchInd {
                        target_reg: *target_reg,
                        switch_targets: vec![],
                    }];
                    i.branch_target = None;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
        let mut reached = BTreeSet::new();
        let mut pending = BTreeSet::from([entry]);
        while let Some(pc) = pending.pop_first() {
            if !reached.insert(pc) {
                continue;
            }
            if let Some(i) = decoded.get(&pc) {
                pending.extend(successors(i));
            }
        }
        decoded.retain(|pc, _| reached.contains(pc));
    }
}
