//! Hopper-Style BasicBlock Control Flow Graph, CFG Minimap, Tags/Colors, and
//! 4-Mode Unified Renderer (`[ASM | CFG | Pseudocode | Hex]`).

use crate::pcode_sleigh::{
    decode_and_lift_arm64, decompile_instructions_to_pseudocode, DecodedInstruction,
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
        let proc_name = name.into();
        seg.set_name_at_address(entry_address, proc_name.clone());

        let mut decoded = Vec::new();
        let mut block_leaders: BTreeSet<u64> = BTreeSet::new();
        block_leaders.insert(entry_address);

        let mut cursor = entry_address;
        for idx in 0..max_instructions {
            let Some(word) = seg.read_u32_le(cursor) else {
                break;
            };
            let insn = decode_and_lift_arm64(cursor, word, seg, objc_stubs, switch_hints);

            if idx == 0 {
                seg.mark_as_procedure(cursor, 4);
            } else {
                seg.mark_as_code(cursor, 4);
            }

            if let Some(target) = insn.branch_target {
                seg.add_reference(cursor, target);
                if !insn.is_call {
                    block_leaders.insert(target);
                    block_leaders.insert(cursor + 4);
                }
            }

            let is_ret = insn.is_ret;
            decoded.push(insn);
            cursor += 4;
            if is_ret {
                break;
            }
        }

        // Partition instructions into Hopper BasicBlocks
        let mut basic_blocks = Vec::new();
        let mut current_insns = Vec::new();
        let mut current_start = entry_address;

        for insn in decoded {
            if !current_insns.is_empty() && block_leaders.contains(&insn.address) {
                let last_addr = insn.address;
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
            let succs = compute_successors(&current_insns, None);
            basic_blocks.push(BasicBlock {
                index: basic_blocks.len(),
                start_address: current_start,
                end_address: end_addr,
                instructions: current_insns,
                successors: succs,
                tags: BTreeSet::new(),
            });
        }

        Self {
            name: proc_name,
            entry_address,
            basic_blocks,
            tags: BTreeSet::new(),
            register_renames: BTreeMap::new(),
        }
    }

    /// Renders Hopper's 4-Mode Unified View (`ASM`, `CFG` + Minimap, `Pseudocode`, `Hex`).
    pub fn render_four_modes(&self, seg: &Segment) -> String {
        let mut out = String::new();

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
        out.push_str(&decompile_instructions_to_pseudocode(
            &self.name,
            self.entry_address,
            &all_insns,
            seg,
        ));

        // MODE 4: Synchronized Hex + Byte-Type Inspector View
        out.push_str("\n=== [Mode 4: Synchronized Hex & ByteType Inspector] ===\n");
        let total_len = self
            .basic_blocks
            .last()
            .map(|b| (b.end_address - self.entry_address) as usize)
            .unwrap_or(16)
            .min(64);
        if let Some(raw) = seg.read_bytes(self.entry_address, total_len) {
            for (chunk_idx, chunk) in raw.chunks(16).enumerate() {
                let row_addr = self.entry_address + (chunk_idx * 16) as u64;
                let hex_cols: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                let ascii_cols: String = chunk
                    .iter()
                    .map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '.' })
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
    if let Some(crate::pcode_sleigh::PcodeOp::BranchInd { switch_targets, .. }) = last.pcode.first()
    {
        return switch_targets.clone();
    }
    if let Some(target) = last.branch_target {
        if !last.is_call {
            out.push(target);
        }
    }
    if !last.is_unconditional_jump {
        if let Some(ft) = fallthrough {
            if !out.contains(&ft) {
                out.push(ft);
            }
        }
    }
    out
}
