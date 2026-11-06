//! Hopper-Style BasicBlock Control Flow Graph, CFG Minimap, Tags/Colors, and
//! 4-Mode Unified Renderer (`[ASM | CFG | Pseudocode | Hex]`).

use crate::pcode_sleigh::{
    decode_and_lift_arm64, decompile_instructions_to_pseudocode_with_aliases, DecodedInstruction,
    SwitchHintTable,
};
use crate::segment::{Segment, Tag};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub index: usize,
    pub start_address: u64,
    pub end_address: u64,
    pub instructions: Vec<DecodedInstruction>,
    pub successors: Vec<u64>,
    pub tags: BTreeSet<Tag>,
}

#[derive(Debug, Clone)]
pub struct Procedure {
    /// Lifted instructions, including discarded paths and bounded callee proofs.
    pub analysis_work: usize,
    pub nonreturning_proofs: BTreeMap<u64, (usize, bool)>,
    pub name: String,
    pub entry_address: u64,
    pub basic_blocks: Vec<BasicBlock>,
    pub tags: BTreeSet<Tag>,
    pub register_renames: BTreeMap<u64, String>,
}

impl Procedure {
    /// Builds a Procedure and its BasicBlock Control Flow Graph starting at `entry_address`
    /// while updating Hopper byte states (`TYPE_PROCEDURE`, `TYPE_CODE`, `TYPE_NEXT`) in `seg`.
    pub fn analyze_arm64(
        name: impl Into<String>,
        entry_address: u64,
        max_instructions: usize,
        seg: &mut Segment,
        objc_stubs: &BTreeMap<u64, String>,
        switch_hints: &SwitchHintTable,
    ) -> Self {
        Self::analyze_arm64_with_imports(
            name,
            entry_address,
            max_instructions,
            seg,
            objc_stubs,
            switch_hints,
            &BTreeMap::new(),
        )
    }

    pub(crate) fn analyze_arm64_with_imports(
        name: impl Into<String>,
        entry_address: u64,
        max_instructions: usize,
        seg: &mut Segment,
        objc_stubs: &BTreeMap<u64, String>,
        switch_hints: &SwitchHintTable,
        nonreturning: &BTreeMap<u64, String>,
    ) -> Self {
        let proc_name = name.into();
        seg.set_name_at_address(entry_address, proc_name.clone());

        let mut decoded = BTreeMap::new();

        let mut pending = BTreeSet::from([entry_address]);
        loop {
            while decoded.len() < max_instructions {
                let Some(cursor) = pending.pop_first() else {
                    break;
                };
                if decoded.contains_key(&cursor) || cursor % 4 != 0 {
                    continue;
                }
                let Some(next) = cursor.checked_add(4) else {
                    continue;
                };
                let Some(word) = seg.read_u32_le(cursor) else {
                    continue;
                };
                let mut insn = decode_and_lift_arm64(cursor, word, seg, objc_stubs, switch_hints);
                if insn.is_call {
                    if let Some((target, symbol)) = insn
                        .branch_target
                        .and_then(|target| nonreturning.get(&target).map(|name| (target, name)))
                    {
                        insn.pcode = vec![crate::pcode_sleigh::PcodeOp::NoReturnCall {
                            target,
                            symbol: symbol.clone(),
                        }];
                        insn.is_unconditional_jump = true;
                    }
                }
                if matches!(
                    insn.pcode.first(),
                    Some(crate::pcode_sleigh::PcodeOp::Unsupported { .. })
                ) {
                    decoded.insert(cursor, insn);
                    continue;
                }

                if let Some(crate::pcode_sleigh::PcodeOp::BranchInd { switch_targets, .. }) =
                    insn.pcode.first()
                {
                    for &target in switch_targets {
                        pending.insert(target);
                    }
                } else if !insn.is_call {
                    if let Some(target) = insn.branch_target {
                        pending.insert(target);
                    }
                }
                if !insn.is_ret && !insn.is_unconditional_jump {
                    if seg.allows_fallthrough(cursor, next) {
                        pending.insert(next);
                    } else {
                        insn.pcode
                            .push(crate::pcode_sleigh::PcodeOp::UnverifiedContinuation {
                                target: next,
                            });
                    }
                }
                decoded.insert(cursor, insn);
            }
            // Wait for direct-flow exploration to settle before proving reaching
            // definitions. Recovered cases share the original instruction budget.
            if !pending.is_empty() || decoded.len() >= max_instructions {
                break;
            }
            let mut added = false;
            for (pc, op) in crate::arm64_jump_table::recover(&decoded, entry_address, seg) {
                let insn = decoded.get_mut(&pc).unwrap();
                if matches!(
                    insn.pcode.first(),
                    Some(crate::pcode_sleigh::PcodeOp::TableBranch { .. })
                ) {
                    continue;
                }
                if let crate::pcode_sleigh::PcodeOp::TableBranch { targets, .. } = &op {
                    pending.extend(targets);
                    insn.branch_target = targets.first().copied();
                }
                insn.pcode = vec![op];
                added = true;
            }
            if !added {
                break;
            }
        }
        let analysis_work = decoded.len();
        crate::arm64_jump_table::validate(&mut decoded, entry_address, seg);
        // Publish code states, references, and leaders only after late-edge
        // validation. Duplicate table entries remain cases, but edges are unique.
        let mut block_leaders = BTreeSet::from([entry_address]);
        for (&pc, insn) in &decoded {
            if matches!(
                insn.pcode.first(),
                Some(crate::pcode_sleigh::PcodeOp::Unsupported { .. })
            ) {
                continue;
            }
            if pc == entry_address {
                seg.mark_as_procedure(pc, 4);
            } else {
                seg.mark_as_code(pc, 4);
            }
            let indirect_targets = match insn.pcode.first() {
                Some(crate::pcode_sleigh::PcodeOp::TableBranch { targets, .. }) => Some(targets),
                Some(crate::pcode_sleigh::PcodeOp::BranchInd { switch_targets, .. }) => {
                    Some(switch_targets)
                }
                _ => None,
            };
            if let Some(targets) = indirect_targets {
                for &target in targets {
                    block_leaders.insert(target);
                    seg.add_reference(pc, target);
                }
            } else if let Some(target) = insn.branch_target {
                seg.add_reference(pc, target);
                if !insn.is_call {
                    block_leaders.insert(target);
                }
            }
            if insn.is_ret
                || insn.is_conditional_jump
                || insn.is_unconditional_jump
                || insn.pcode.iter().any(|op| {
                    matches!(
                        op,
                        crate::pcode_sleigh::PcodeOp::UnverifiedContinuation { .. }
                    )
                })
            {
                block_leaders.extend(pc.checked_add(4));
            }
        }

        // Partition instructions into Hopper BasicBlocks
        let mut basic_blocks = Vec::new();
        let mut current_insns = Vec::new();
        let mut current_start = entry_address;

        for insn in decoded.into_values() {
            if current_insns.is_empty() {
                current_start = insn.address;
            }
            if !current_insns.is_empty()
                && (block_leaders.contains(&insn.address)
                    || current_insns
                        .last()
                        .map(|i: &DecodedInstruction| i.address + 4)
                        != Some(insn.address))
            {
                let last_addr = current_insns.last().unwrap().address + 4;
                let succs = compute_successors(&current_insns, Some(last_addr));
                basic_blocks.push(BasicBlock {
                    index: basic_blocks.len(),
                    start_address: current_start,
                    end_address: last_addr,
                    instructions: std::mem::take(&mut current_insns),
                    successors: succs,
                    tags: BTreeSet::new(),
                });
                current_start = insn.address;
            }
            current_insns.push(insn);
        }

        if !current_insns.is_empty() {
            let end_addr = current_insns
                .last()
                .map(|i| i.address + 4)
                .unwrap_or(current_start);
            let succs = compute_successors(&current_insns, Some(end_addr));
            basic_blocks.push(BasicBlock {
                index: basic_blocks.len(),
                start_address: current_start,
                end_address: end_addr,
                instructions: current_insns,
                successors: succs,
                tags: BTreeSet::new(),
            });
        }

        let mut proc = Self {
            analysis_work,
            nonreturning_proofs: BTreeMap::new(),
            name: proc_name,
            entry_address,
            basic_blocks,
            tags: BTreeSet::new(),
            register_renames: BTreeMap::new(),
        };
        if proc.name.starts_with("-[") || proc.name.starts_with("+[") {
            proc.apply_objc_abi_aliases();
        }
        proc
    }

    /// Hopper `renameRegister` equivalent: assigns a semantic variable name to an ARM64 register.
    pub fn rename_register(&mut self, reg_idx: u64, alias: impl Into<String>) {
        self.register_renames.insert(reg_idx, alias.into());
    }

    /// Hopper Objective-C ABI auto-aliasing (`x0 -> self`, `x1 -> _cmd`).
    pub fn apply_objc_abi_aliases(&mut self) {
        self.register_renames
            .entry(0)
            .or_insert_with(|| "self".into());
        self.register_renames
            .entry(1)
            .or_insert_with(|| "_cmd".into());
    }

    pub fn is_incomplete(&self) -> bool {
        let starts: BTreeSet<_> = self
            .basic_blocks
            .iter()
            .map(|bb| bb.start_address)
            .collect();
        self.basic_blocks.is_empty()
            || self.basic_blocks.iter().any(|bb| {
                bb.instructions.iter().any(|i| {
                    i.pcode.iter().any(|op| {
                        matches!(
                            op,
                            crate::pcode_sleigh::PcodeOp::Unsupported { .. }
                                | crate::pcode_sleigh::PcodeOp::UnverifiedContinuation { .. }
                        )
                    })
                }) || bb.successors.iter().any(|target| !starts.contains(target))
                    || bb.instructions.last().is_some_and(|i| {
                        i.is_unconditional_jump
                            && i.branch_target.is_none()
                            && !matches!(
                                i.pcode.first(),
                                Some(
                                    crate::pcode_sleigh::PcodeOp::ExternalTailCall { .. }
                                        | crate::pcode_sleigh::PcodeOp::RuntimeTailCall { .. }
                                        | crate::pcode_sleigh::PcodeOp::MachOImportTailCall { .. }
                                        | crate::pcode_sleigh::PcodeOp::RuntimeJniCall {
                                            tail: true,
                                            ..
                                        }
                                )
                            )
                    })
            })
    }

    /// A complete overapproximation with no normal return and only trusted
    /// nonreturning terminal calls. Unknown boundaries/branches cannot prove it.
    pub(crate) fn proves_no_normal_return(&self) -> bool {
        !self.is_incomplete()
            && !self
                .basic_blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .any(|i| i.is_ret)
            && self.basic_blocks.iter().any(|b| b.successors.is_empty())
            && self
                .basic_blocks
                .iter()
                .filter(|b| b.successors.is_empty())
                .all(|b| {
                    b.instructions.last().is_some_and(|i| {
                        matches!(
                            i.pcode.first(),
                            Some(crate::pcode_sleigh::PcodeOp::NoReturnCall { .. })
                        )
                    })
                })
    }

    /// Reuse lifted caller nodes after proving a callee. No extra instruction
    /// budget is hidden by re-decoding the caller, and unreachable nodes vanish.
    pub(crate) fn apply_nonreturning(&mut self, targets: &BTreeSet<u64>) {
        for bb in &mut self.basic_blocks {
            if let Some(n) = bb
                .instructions
                .iter()
                .position(|i| i.is_call && i.branch_target.is_some_and(|t| targets.contains(&t)))
            {
                let i = &mut bb.instructions[n];
                let target = i.branch_target.unwrap();
                i.pcode = vec![crate::pcode_sleigh::PcodeOp::NoReturnCall {
                    target,
                    symbol: format!("proven_nonreturning_0x{target:x}"),
                }];
                i.is_unconditional_jump = true;
                bb.end_address = i.address + 4;
                bb.successors.clear();
                bb.instructions.truncate(n + 1);
            }
        }
        let blocks: BTreeMap<_, _> = self
            .basic_blocks
            .iter()
            .map(|b| (b.start_address, b.successors.clone()))
            .collect();
        let mut reached = BTreeSet::new();
        let mut pending = BTreeSet::from([self.entry_address]);
        while let Some(pc) = pending.pop_first() {
            if reached.insert(pc) {
                if let Some(next) = blocks.get(&pc) {
                    pending.extend(next);
                }
            }
        }
        self.basic_blocks
            .retain(|b| reached.contains(&b.start_address));
        for (n, b) in self.basic_blocks.iter_mut().enumerate() {
            b.index = n;
        }
    }

    /// Renders the partial analysis, with explicit completeness status.
    pub fn render_four_modes(&self, seg: &Segment) -> String {
        let mut out = String::new();
        out.push_str(if self.is_incomplete() {"Analysis status: INCOMPLETE (unsupported instruction, unresolved edge, or instruction budget)\n"} else {"Analysis status: supported subset only; full semantic equivalence is unverified\n"});
        if self
            .basic_blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|i| {
                i.pcode
                    .iter()
                    .any(|op| matches!(op, crate::pcode_sleigh::PcodeOp::RuntimeJniCall { .. }))
            })
        {
            out.push_str("Runtime JNI scope: supplied offline capture; stable JNIEnv table assumed; external ART body not analyzed\n");
        }
        if self
            .basic_blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|i| {
                i.pcode.iter().any(|op| {
                    matches!(
                        op,
                        crate::pcode_sleigh::PcodeOp::RuntimeTailCall { .. }
                            | crate::pcode_sleigh::PcodeOp::RuntimeInvalidBranch { .. }
                    )
                })
            })
        {
            out.push_str("Runtime branch scope: supplied controlled replay only; callback interpretation conditional on entry x0; external body not analyzed; observed faults remain INCOMPLETE\n");
        }
        out.push_str(&format!(
            "Instruction work: {} (caller plus bounded callee proofs)\n",
            self.analysis_work
        ));
        for (&target, &(work, proven)) in &self.nonreturning_proofs {
            out.push_str(&format!(
                "Nonreturn proof: 0x{target:x}, {work} instructions, proven={proven}\n"
            ));
        }

        // MODE 1: Assembly View (with ByteType, XREFs, ObjC selectors, and inline comments)
        out.push_str("=== [Mode 1: Hopper Assembly + Byte-State View] ===\n");
        for bb in &self.basic_blocks {
            out.push_str(&format!(
                "; BasicBlock #{} (0x{:x} .. 0x{:x}) succs={:?}\n",
                bb.index, bb.start_address, bb.end_address, bb.successors
            ));
            for insn in &bb.instructions {
                let btype = seg.get_type_at_address(insn.address).as_str();
                let args = insn.formatted_args.join(", ");
                let xrefs: Vec<String> = seg
                    .refs_to
                    .get(&insn.address)
                    .map(|set| set.iter().map(|a| format!("0x{:x}", a)).collect())
                    .unwrap_or_default();
                let xref_note = if xrefs.is_empty() {
                    String::new()
                } else {
                    format!(" ; XREF: {}", xrefs.join(", "))
                };
                out.push_str(&format!(
                    "0x{:09x} [{:<9}]  {:02x} {:02x} {:02x} {:02x}   {:<8} {:<28}{}\n",
                    insn.address,
                    btype,
                    insn.bytes[0],
                    insn.bytes[1],
                    insn.bytes[2],
                    insn.bytes[3],
                    insn.mnemonic,
                    args,
                    xref_note
                ));
            }
        }

        // MODE 2: Control Flow Graph (CFG) + Minimap (`ICCFGMiniMapViewController` equivalent)
        out.push_str("\n=== [Mode 2: Control Flow Graph (CFG) & Minimap] ===\n");
        for bb in &self.basic_blocks {
            let tag_list: Vec<&str> = bb.tags.iter().map(|t| t.name.as_str()).collect();
            out.push_str(&format!(
                "┌── [BB#{} @ 0x{:x}] tags={:?} ──┐\n",
                bb.index, bb.start_address, tag_list
            ));
            for insn in &bb.instructions {
                out.push_str(&format!(
                    "│ 0x{:x}: {} {}\n",
                    insn.address,
                    insn.mnemonic,
                    insn.formatted_args.join(", ")
                ));
            }
            let edges: Vec<String> = bb
                .successors
                .iter()
                .map(|s| format!("-> 0x{:x}", s))
                .collect();
            out.push_str(&format!("└── Edges: {} ──┘\n", edges.join(" | ")));
        }

        // MODE 3: Hybrid P-Code + ObjC Decompiler Pseudocode
        out.push_str("\n=== [Mode 3: Ghidra P-Code + Hopper ObjC Decompiler] ===\n");
        let all_insns: Vec<DecodedInstruction> = self
            .basic_blocks
            .iter()
            .flat_map(|b| b.instructions.iter().cloned())
            .collect();
        out.push_str(&decompile_instructions_to_pseudocode_with_aliases(
            &self.name,
            self.entry_address,
            &all_insns,
            seg,
            &self.register_renames,
        ));

        // MODE 4: Synchronized Hex + Byte-Type Inspector View
        out.push_str("\n=== [Mode 4: Synchronized Hex & ByteType Inspector] ===\n");
        let total_len = self
            .basic_blocks
            .last()
            .map(|b| b.end_address.saturating_sub(self.entry_address) as usize)
            .unwrap_or(16)
            .min(64);
        if let Some(raw) = seg.read_bytes(self.entry_address, total_len) {
            for (chunk_idx, chunk) in raw.chunks(16).enumerate() {
                let row_addr = self.entry_address + (chunk_idx * 16) as u64;
                let hex_cols: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                let ascii_cols: String = chunk
                    .iter()
                    .map(|&b| {
                        if (0x20..=0x7e).contains(&b) {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                out.push_str(&format!(
                    "0x{:09x}  {:<47}  |{}|\n",
                    row_addr,
                    hex_cols.join(" "),
                    ascii_cols
                ));
            }
        }

        out
    }
}

fn compute_successors(insns: &[DecodedInstruction], fallthrough: Option<u64>) -> Vec<u64> {
    let Some(last) = insns.last() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if last.is_ret {
        return out;
    }
    if matches!(
        last.pcode.first(),
        Some(crate::pcode_sleigh::PcodeOp::Unsupported { .. })
    ) {
        return out;
    }
    if let Some(crate::pcode_sleigh::PcodeOp::BranchInd { switch_targets, .. }) = last.pcode.first()
    {
        return switch_targets.clone();
    }
    if let Some(crate::pcode_sleigh::PcodeOp::TableBranch { targets, .. }) = last.pcode.first() {
        return targets
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    if let Some(target) = last.branch_target {
        if !last.is_call {
            out.push(target);
        }
    }
    if !last.is_unconditional_jump
        && !last.pcode.iter().any(|op| {
            matches!(
                op,
                crate::pcode_sleigh::PcodeOp::UnverifiedContinuation { .. }
            )
        })
    {
        if let Some(ft) = fallthrough {
            if !out.contains(&ft) {
                out.push(ft);
            }
        }
    }
    out
}
