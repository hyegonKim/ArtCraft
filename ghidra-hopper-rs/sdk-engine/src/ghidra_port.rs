//! Direct 1:1 Rust Port of Ghidra 12.3_DEV's `ObjcMessageAnalyzer.java`,
//! `DecompilerSwitchAnalysisCmd.java`, and our `HopperHybridObjcAnalyzer.java`.
//!
//! Eliminates Ghidra's Java-to-C++ `DecompileProcess` stdio IPC pipe and JVM heap bloat
//! while preserving Ghidra's exact domain model (`Program`, `MemoryBlock`, `ReferenceManager`,
//! `RefType::CallOverrideUnconditional`, `RefType::ComputedJump`, `JumpTable`, `BookmarkManager`)
//! fused with Hopper Disassembler's O(1) `ByteType` array and `CALL_OBJC` selector recovery.

use crate::cfg_tags::Procedure;
use crate::macho_objc::MachOImage;
use crate::pcode_sleigh::SwitchHintTable;
use crate::segment::{ByteType, Segment, Tag};
use std::collections::BTreeMap;

/// Port of `ghidra.program.model.symbol.RefType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhidraRefType {
    UnconditionalCall,
    CallOverrideUnconditional,
    ComputedJump,
    Data,
}

/// Port of `ghidra.program.model.symbol.SourceType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhidraSourceType {
    Default,
    Analysis,
    Imported,
    UserDefined,
}

/// Port of `ghidra.program.model.symbol.Reference`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhidraReference {
    pub from_addr: u64,
    pub to_addr: u64,
    pub ref_type: GhidraRefType,
    pub source: GhidraSourceType,
    pub op_index: i32,
}

/// Port of `ghidra.program.model.listing.Bookmark`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhidraBookmark {
    pub address: u64,
    pub bookmark_type: String,
    pub category: String,
    pub comment: String,
}

/// Port of `ghidra.program.model.pcode.JumpTable` + `DecompilerSwitchAnalysisCmd`.
#[derive(Debug, Clone)]
pub struct GhidraJumpTable {
    pub switch_address: u64,
    pub case_targets: Vec<u64>,
    pub label_values: Vec<i32>,
    pub default_case_value: i32,
}

impl GhidraJumpTable {
    pub const DEFAULT_CASE_VALUE: i32 = 0xbad1abe1u32 as i32;

    pub fn new(switch_address: u64, case_targets: Vec<u64>) -> Self {
        let label_values = (0..case_targets.len() as i32).collect();
        Self {
            switch_address,
            case_targets,
            label_values,
            default_case_value: Self::DEFAULT_CASE_VALUE,
        }
    }

    /// Port of `JumpTable.writeOverride(Function)` + `DecompilerSwitchAnalysisCmd.processBranchIND`.
    pub fn write_override(&self, prog: &mut GhidraProgram) {
        prog.switch_hints
            .hints
            .insert(self.switch_address, self.case_targets.clone());

        for (idx, &dest) in self.case_targets.iter().enumerate() {
            prog.add_memory_reference(
                self.switch_address,
                dest,
                GhidraRefType::ComputedJump,
                GhidraSourceType::Analysis,
                -1,
            );
            let case_label = format!("switchD_{:x}::caseD_{}", self.switch_address, idx);
            prog.symbol_table.insert(dest, case_label);
            if let Some(seg) = prog.segment_at_mut(self.switch_address) {
                seg.add_reference(self.switch_address, dest);
            }
        }

        prog.bookmarks.push(GhidraBookmark {
            address: self.switch_address,
            bookmark_type: "HopperTag".into(),
            category: "SwitchOverride".into(),
            comment: format!(
                "JumpTable override with {} cases applied (Zero-IPC)",
                self.case_targets.len()
            ),
        });
    }
}

/// Port of `ObjcMessageAnalyzer.Message` record in Ghidra 12.3_DEV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjcMessage {
    pub receiver: String,
    pub selector: String,
    pub function_entry: u64,
    pub call_addr: u64,
    pub stub_target: u64,
    pub parameters: Vec<String>,
}

/// Port of `ObjcMessageAnalyzer.DataTypes` record in Ghidra 12.3_DEV.
#[derive(Debug, Clone)]
pub struct ObjcDataTypes {
    pub ptr_size: usize,
    pub id_type: &'static str,
    pub sel_type: &'static str,
    pub class_t_type: &'static str,
    pub objc_super_type: &'static str,
}

impl Default for ObjcDataTypes {
    fn default() -> Self {
        Self {
            ptr_size: 8,
            id_type: "ID",
            sel_type: "SEL",
            class_t_type: "class_t",
            objc_super_type: "objc_super*",
        }
    }
}

/// Unified Rust representation of Ghidra's `ProgramDB` + Hopper's `Document`.
#[derive(Debug, Clone)]
pub struct GhidraProgram {
    pub selector_stub_proofs: BTreeMap<u64, crate::macho_objc::SelectorStubProof>,
    pub macho_imports: BTreeMap<u64, crate::macho_objc::MachOImportBinding>,
    pub plt_bindings: BTreeMap<u64, crate::macho_objc::PltBinding>,
    pub name: String,
    pub segments: Vec<Segment>,
    pub symbol_table: BTreeMap<u64, String>,
    pub external_methods: BTreeMap<(String, String), u64>,
    pub references: Vec<GhidraReference>,
    pub eol_comments: BTreeMap<u64, String>,
    pub bookmarks: Vec<GhidraBookmark>,
    pub objc_stub_selectors: BTreeMap<u64, String>,
    pub switch_hints: SwitchHintTable,
    pub data_types: ObjcDataTypes,
}

impl GhidraProgram {
    pub fn new(name: impl Into<String>, text_seg: Segment) -> Self {
        Self {
            macho_imports: BTreeMap::new(),
            selector_stub_proofs: BTreeMap::new(),
            plt_bindings: BTreeMap::new(),
            name: name.into(),
            segments: vec![text_seg],
            symbol_table: BTreeMap::new(),
            external_methods: BTreeMap::new(),
            references: Vec::new(),
            eol_comments: BTreeMap::new(),
            bookmarks: Vec::new(),
            objc_stub_selectors: BTreeMap::new(),
            switch_hints: SwitchHintTable::default(),
            data_types: ObjcDataTypes::default(),
        }
    }

    pub fn from_macho_image(name: impl Into<String>, img: &MachOImage) -> Self {
        let mut prog = Self {
            selector_stub_proofs: img.objc.selector_stub_proofs.clone(),
            macho_imports: img.objc.macho_imports.clone(),
            plt_bindings: img.objc.plt_bindings.clone(),
            name: name.into(),
            segments: img.segments.clone(),
            symbol_table: BTreeMap::new(),
            external_methods: BTreeMap::new(),
            references: Vec::new(),
            eol_comments: BTreeMap::new(),
            bookmarks: Vec::new(),
            objc_stub_selectors: img.objc.selector_stubs.clone(),
            switch_hints: SwitchHintTable::default(),
            data_types: ObjcDataTypes::default(),
        };
        for (&stub_addr, sel) in &prog.objc_stub_selectors {
            prog.symbol_table
                .insert(stub_addr, format!("objc_msgSend${}", sel));
        }
        prog.symbol_table.extend(img.objc.symbols.clone());
        for seg in &mut prog.segments {
            for (&addr, name) in &prog.symbol_table {
                if seg.contains(addr) {
                    seg.set_name_at_address(addr, name.clone());
                }
            }
        }
        prog
    }

    pub fn segment_at(&self, addr: u64) -> Option<&Segment> {
        self.segments.iter().find(|seg| seg.contains(addr))
    }

    pub fn segment_at_mut(&mut self, addr: u64) -> Option<&mut Segment> {
        self.segments.iter_mut().find(|seg| seg.contains(addr))
    }

    pub fn text_segment(&self) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|s| s.name == "__TEXT")
            .or_else(|| self.segments.first())
    }

    pub fn text_segment_mut(&mut self) -> Option<&mut Segment> {
        if let Some(pos) = self.segments.iter().position(|s| s.name == "__TEXT") {
            self.segments.get_mut(pos)
        } else {
            self.segments.first_mut()
        }
    }

    pub fn add_memory_reference(
        &mut self,
        from_addr: u64,
        to_addr: u64,
        ref_type: GhidraRefType,
        source: GhidraSourceType,
        op_index: i32,
    ) {
        self.references.push(GhidraReference {
            from_addr,
            to_addr,
            ref_type,
            source,
            op_index,
        });
    }
}

/// Rust port of `HopperHybridObjcAnalyzer.java` + `ObjcMessageAnalyzer.java`.
#[derive(Debug, Default)]
pub struct HopperHybridObjcAnalyzerRs {
    pub use_call_overrides: bool,
    pub discovered_messages: Vec<ObjcMessage>,
}

impl HopperHybridObjcAnalyzerRs {
    pub fn new() -> Self {
        Self {
            use_call_overrides: true,
            discovered_messages: Vec::new(),
        }
    }

    /// Port of `HopperHybridObjcAnalyzer.added(Program, AddressSetView, TaskMonitor, MessageLog)`
    /// combined with `ObjcMessageAnalyzer.fixMsgSendCalls` and `DecompilerSwitchAnalysisCmd`.
    pub fn analyze_function_zero_ipc(
        &mut self,
        prog: &mut GhidraProgram,
        func_name: &str,
        entry_addr: u64,
        max_insns: usize,
        jump_tables: &[GhidraJumpTable],
    ) -> Procedure {
        // 1. Apply JumpTable / SwitchHint overrides first (DecompilerSwitchAnalysisCmd)
        for jt in jump_tables {
            jt.write_override(prog);
        }

        // 2. Decode & Lift ARM64 instructions + build Hopper ByteType states & BasicBlock CFG
        let mut stubs = prog.objc_stub_selectors.clone();
        let selector_context =
            if prog.selector_stub_proofs.is_empty() && prog.macho_imports.is_empty() {
                String::new()
            } else {
                crate::macho_objc::selector_context(&prog.segments)
            };
        stubs.retain(|&pc, name| {
            prog.selector_stub_proofs.get(&pc).is_none_or(|proof| {
                proof.selector == *name
                    && proof.matches_with_context(pc, &prog.segments, &selector_context)
            })
        });
        let hints = prog.switch_hints.clone();
        let mut nonreturning = BTreeMap::new();
        for binding in prog
            .plt_bindings
            .values()
            .filter(|b| b.symbol == "__stack_chk_fail")
        {
            if binding.instructions.iter().enumerate().all(|(n, word)| {
                prog.segment_at(binding.stub_address)
                    .and_then(|s| s.read_u32_le(binding.stub_address + n as u64 * 4))
                    == Some(*word)
            }) {
                nonreturning.insert(binding.stub_address, binding.symbol.clone());
            }
        }
        for binding in prog.macho_imports.values().filter(|b| {
            b.context_sha256 == selector_context
                && matches!(b.symbol.as_str(), "___stack_chk_fail" | "_exit")
                && b.library == "/usr/lib/libSystem.B.dylib"
        }) {
            if binding.instructions.iter().enumerate().all(|(n, word)| {
                prog.segment_at(binding.stub_address)
                    .and_then(|s| s.read_u32_le(binding.stub_address + n as u64 * 4))
                    == Some(*word)
            }) {
                nonreturning.insert(binding.stub_address, binding.symbol.clone());
            }
        }
        let original_segment = prog.segment_at(entry_addr).unwrap().clone();
        let mut proc = {
            let seg = prog
                .segment_at_mut(entry_addr)
                .expect("Function entry must be in a loaded segment");
            Procedure::analyze_arm64_with_imports(
                func_name,
                entry_addr,
                max_insns,
                seg,
                &stubs,
                &hints,
                &nonreturning,
            )
        };
        // Only inspect declared direct callees at unverified boundaries, once,
        // with the unused caller budget. This is a one-level proof, not a guess
        // from the callee name or from its last instruction alone.
        let targets: std::collections::BTreeSet<_> = proc
            .basic_blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter(|i| {
                i.is_call
                    && matches!(
                        i.pcode.first(),
                        Some(crate::pcode_sleigh::PcodeOp::Call { .. })
                    )
                    && i.pcode.iter().any(|op| {
                        matches!(
                            op,
                            crate::pcode_sleigh::PcodeOp::UnverifiedContinuation { .. }
                        )
                    })
            })
            .filter_map(|i| i.branch_target)
            .filter(|&target| {
                target != entry_addr
                    && prog
                        .segment_at(target)
                        .is_some_and(|s| s.function_starts.contains(&target))
            })
            .collect();
        let initial_addresses: std::collections::BTreeSet<_> = proc
            .basic_blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|i| i.address)
            .collect();
        let mut proven = std::collections::BTreeSet::new();
        for target in targets {
            let remaining = max_insns.saturating_sub(proc.analysis_work);
            if remaining == 0 {
                break;
            }
            let mut snapshot = prog.segment_at(target).unwrap().clone();
            let proof = Procedure::analyze_arm64_with_imports(
                "bounded nonreturn proof",
                target,
                remaining,
                &mut snapshot,
                &stubs,
                &hints,
                &nonreturning,
            );
            let success = proof.proves_no_normal_return();
            proc.analysis_work += proof.analysis_work;
            proc.nonreturning_proofs
                .insert(target, (proof.analysis_work, success));
            if success {
                proven.insert(target);
            }
        }
        if !proven.is_empty() {
            proc.apply_nonreturning(&proven);
            let kept: std::collections::BTreeSet<_> = proc
                .basic_blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|i| i.address)
                .collect();
            let seg = prog.segment_at_mut(entry_addr).unwrap();
            for &address in initial_addresses.difference(&kept) {
                let offset = (address - seg.start_address) as usize;
                seg.byte_types[offset..offset + 4]
                    .copy_from_slice(&original_segment.byte_types[offset..offset + 4]);
            }
            seg.refs_from = original_segment.refs_from.clone();
            seg.refs_to = original_segment.refs_to.clone();
            for i in proc.basic_blocks.iter().flat_map(|b| &b.instructions) {
                if let Some(target) = i.branch_target {
                    seg.add_reference(i.address, target);
                }
                match i.pcode.first() {
                    Some(crate::pcode_sleigh::PcodeOp::TableBranch { targets, .. })
                    | Some(crate::pcode_sleigh::PcodeOp::BranchInd {
                        switch_targets: targets,
                        ..
                    }) => {
                        for &t in targets {
                            seg.add_reference(i.address, t);
                        }
                    }
                    _ => {}
                }
            }
        }
        for bb in &mut proc.basic_blocks {
            let decoded_words: BTreeMap<_, _> = bb
                .instructions
                .iter()
                .map(|i| (i.address, u32::from_le_bytes(i.bytes)))
                .collect();
            for insn in &mut bb.instructions {
                if let Some(binding) = prog.macho_imports.get(&insn.address).cloned() {
                    // Entering at LDR/BR, an explicit switch override, or changed
                    // code cannot inherit an import proven for the full stub.
                    let full_stub = binding.context_sha256 == selector_context
                        && binding.instructions.iter().enumerate().all(|(n, word)| {
                            decoded_words.get(&(binding.stub_address + n as u64 * 4)) == Some(word)
                        });
                    if full_stub
                        && matches!(insn.pcode.first(), Some(crate::pcode_sleigh::PcodeOp::BranchInd { target_reg, switch_targets }) if target_reg.offset == 16 && switch_targets.is_empty())
                    {
                        insn.pcode = vec![crate::pcode_sleigh::PcodeOp::MachOImportTailCall {
                            symbol: binding.symbol.clone(),
                            library: binding.library.clone(),
                            pointer_address: binding.pointer_address,
                            selector_address: binding.selector_address,
                            selector: stubs.get(&binding.stub_address).cloned(),
                        }];
                        prog.add_memory_reference(
                            insn.address,
                            binding.pointer_address,
                            GhidraRefType::Data,
                            GhidraSourceType::Analysis,
                            0,
                        );
                        prog.eol_comments.insert(
                            insn.address,
                            format!(
                                "Mach-O import {} from {} via slot 0x{:x}; runtime address unknown",
                                binding.symbol, binding.library, binding.pointer_address
                            ),
                        );
                    }
                }
                if let Some(crate::pcode_sleigh::PcodeOp::TableBranch {
                    table_address,
                    targets,
                    ..
                }) = insn.pcode.first()
                {
                    for target in targets
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                    {
                        prog.add_memory_reference(
                            insn.address,
                            target,
                            GhidraRefType::ComputedJump,
                            GhidraSourceType::Analysis,
                            0,
                        );
                    }
                    prog.add_memory_reference(
                        insn.address,
                        *table_address,
                        GhidraRefType::Data,
                        GhidraSourceType::Analysis,
                        0,
                    );
                    prog.eol_comments.insert(insn.address, format!("Verified u16 relative table at 0x{table_address:x}: {} entries, scale 4", targets.len()));
                }
                if let Some(binding) = prog.plt_bindings.get(&insn.address).cloned() {
                    if !binding.instructions.iter().enumerate().all(|(n, word)| {
                        decoded_words.get(&(binding.stub_address + n as u64 * 4)) == Some(word)
                    }) {
                        continue;
                    }
                    if matches!(insn.pcode.first(),Some(crate::pcode_sleigh::PcodeOp::BranchInd{target_reg,..}) if target_reg.offset==17)
                    {
                        insn.pcode = vec![crate::pcode_sleigh::PcodeOp::ExternalTailCall {
                            symbol: binding.symbol.clone(),
                            got_address: binding.got_address,
                        }];
                        prog.add_memory_reference(
                            insn.address,
                            binding.got_address,
                            GhidraRefType::Data,
                            GhidraSourceType::Analysis,
                            0,
                        );
                        prog.eol_comments.insert(
                            insn.address,
                            format!(
                                "JUMP_SLOT {} via GOT 0x{:x}; runtime address unknown",
                                binding.symbol, binding.got_address
                            ),
                        );
                    }
                }
            }
        }

        // 3. Scan lifted P-Code ops in-memory (Zero-IPC!) to resolve ObjcMessage calls
        //    (replaces Ghidra's slow ParallelDecompiler IPC callback in ObjcMessageAnalyzer.java)
        let mut reg_strings: BTreeMap<u64, String> = BTreeMap::new();
        let mut reg_consts: BTreeMap<u64, u64> = BTreeMap::new();
        let mut pending_messages = Vec::new();

        if let Some(seg) = prog.segment_at(entry_addr) {
            for bb in &proc.basic_blocks {
                // A linear scan is not a dataflow merge. Do not propagate values from
                // one branch into a different path or through unknown instructions.
                reg_strings.clear();
                reg_consts.clear();
                for insn in &bb.instructions {
                    if insn.mnemonic == "inst.arm64" {
                        reg_strings.clear();
                        reg_consts.clear();
                    }
                    for op in &insn.pcode {
                        match op {
                            crate::pcode_sleigh::PcodeOp::ShiftedAdd { dst: Some(dst), .. } => {
                                reg_consts.remove(&dst.offset);
                                reg_strings.remove(&dst.offset);
                            }
                            crate::pcode_sleigh::PcodeOp::IntOr { dst, .. } => {
                                if dst.space == crate::pcode_sleigh::SpaceKind::Register {
                                    reg_consts.remove(&dst.offset);
                                    reg_strings.remove(&dst.offset);
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::FlagArithmetic {
                                dst: Some(dst), ..
                            } => {
                                reg_consts.remove(&dst.offset);
                                reg_strings.remove(&dst.offset);
                            }
                            crate::pcode_sleigh::PcodeOp::Bitfield { dst, .. } => {
                                if dst.space == crate::pcode_sleigh::SpaceKind::Register {
                                    reg_consts.remove(&dst.offset);
                                    reg_strings.remove(&dst.offset);
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::Copy { dst, src } => {
                                if dst.space == crate::pcode_sleigh::SpaceKind::Register {
                                    let value = if src.space
                                        == crate::pcode_sleigh::SpaceKind::Constant
                                    {
                                        Some(src.offset)
                                    } else if src.space == crate::pcode_sleigh::SpaceKind::Register
                                    {
                                        reg_consts.get(&src.offset).copied()
                                    } else {
                                        None
                                    };
                                    reg_consts.remove(&dst.offset);
                                    if let Some(value) = value.map(|value| {
                                        value
                                            & if dst.size == 4 {
                                                u32::MAX as u64
                                            } else {
                                                u64::MAX
                                            }
                                    }) {
                                        reg_consts.insert(dst.offset, value);
                                    }
                                    reg_strings.remove(&dst.offset);
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::IntAdd { dst, lhs, rhs } => {
                                if dst.space != crate::pcode_sleigh::SpaceKind::Register {
                                    continue;
                                }
                                let previous = reg_consts.get(&lhs.offset).copied();
                                reg_consts.remove(&dst.offset);
                                reg_strings.remove(&dst.offset);
                                if let Some(base) = previous.filter(|_| {
                                    lhs.space == crate::pcode_sleigh::SpaceKind::Register
                                        && rhs.space == crate::pcode_sleigh::SpaceKind::Constant
                                }) {
                                    let target = base.wrapping_add(rhs.offset)
                                        & if dst.size == 4 {
                                            u32::MAX as u64
                                        } else {
                                            u64::MAX
                                        };
                                    reg_consts.insert(dst.offset, target);
                                    if let Some(s) = crate::pcode_sleigh::read_ascii_at(seg, target)
                                    {
                                        reg_strings.insert(dst.offset, s);
                                    }
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::Load { dst, .. }
                            | crate::pcode_sleigh::PcodeOp::ZeroExtend { dst, .. } => {
                                if dst.space == crate::pcode_sleigh::SpaceKind::Register {
                                    reg_consts.remove(&dst.offset);
                                    reg_strings.remove(&dst.offset);
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::CallObjC {
                                stub_addr,
                                selector,
                            } => {
                                let receiver =
                                    if func_name.starts_with("-[") || func_name.starts_with("+[") {
                                        "self"
                                    } else {
                                        "x0"
                                    }
                                    .to_string();
                                let arg = reg_strings
                                    .get(&2)
                                    .map(|s| format!("{s:?}"))
                                    .unwrap_or_else(|| "x2".to_string());
                                pending_messages.push(ObjcMessage {
                                    receiver,
                                    selector: selector.clone(),
                                    function_entry: entry_addr,
                                    call_addr: insn.address,
                                    stub_target: *stub_addr,
                                    parameters: (0..selector.matches(':').count())
                                        .map(|i| {
                                            if i == 0 {
                                                arg.clone()
                                            } else if i < 6 {
                                                format!("x{}", i + 2)
                                            } else {
                                                format!("unknown_stack_arg_{i}")
                                            }
                                        })
                                        .collect(),
                                });
                            }
                            _ => {
                                if let Some(dst) = op
                                    .value_destination()
                                    .filter(|d| d.space == crate::pcode_sleigh::SpaceKind::Register)
                                {
                                    reg_consts.remove(&dst.offset);
                                    reg_strings.remove(&dst.offset);
                                }
                            }
                        }
                    }
                    if insn.is_call {
                        reg_consts.retain(|reg, _| *reg > 18);
                        reg_strings.retain(|reg, _| *reg > 18);
                    }
                    if let Some(crate::pcode_sleigh::PcodeOp::IntSub { dst, .. }) =
                        insn.pcode.first()
                    {
                        reg_consts.remove(&dst.offset);
                        reg_strings.remove(&dst.offset);
                    }
                }
            }
        }

        // 4. Port of `updateExternalBlock` + `setCommentAndReference` in `ObjcMessageAnalyzer.java`
        for msg in pending_messages {
            self.set_comment_and_reference(prog, &msg);
            self.discovered_messages.push(msg);
        }

        if let Some(first_bb) = proc.basic_blocks.first_mut() {
            first_bb
                .tags
                .insert(Tag::new("ghidra-objc-override", "#4ECDC4"));
        }

        proc
    }

    /// Port of `ObjcMessageAnalyzer.setCommentAndReference(Program, Message, List<String>, MessageLog)`.
    fn set_comment_and_reference(&self, prog: &mut GhidraProgram, msg: &ObjcMessage) {
        let key = (msg.receiver.clone(), msg.selector.clone());
        let known_target = prog
            .external_methods
            .get(&key)
            .copied()
            .filter(|addr| prog.segment_at(*addr).is_some());
        let target_addr = if self.use_call_overrides {
            known_target.unwrap_or(msg.stub_target)
        } else {
            msg.stub_target
        };

        let ref_type = if self.use_call_overrides && known_target.is_some() {
            GhidraRefType::CallOverrideUnconditional
        } else {
            GhidraRefType::UnconditionalCall
        };
        prog.add_memory_reference(
            msg.call_addr,
            target_addr,
            ref_type,
            GhidraSourceType::Analysis,
            0,
        );

        // Format Objective-C bracket expression exactly like ObjcMessageAnalyzer.java lines 491-505
        let formatted_comment = if msg.selector.contains(':') {
            let parts: Vec<&str> = msg.selector.split(':').collect();
            let mut buf = format!("[{} {}", msg.receiver, parts[0]);
            for (i, part) in parts.iter().skip(1).enumerate() {
                if i == parts.len() - 2 && part.is_empty() {
                    let p = msg
                        .parameters
                        .get(i)
                        .map(String::as_str)
                        .unwrap_or("<<unknown>>");
                    buf.push_str(&format!(":{}]", p));
                } else {
                    let p = msg
                        .parameters
                        .get(i)
                        .map(String::as_str)
                        .unwrap_or("<<unknown>>");
                    buf.push_str(&format!(":{} {}", p, part));
                }
            }
            buf
        } else {
            format!("[{} {}]", msg.receiver, msg.selector)
        };

        prog.eol_comments
            .insert(msg.call_addr, formatted_comment.clone());
        if let Some(seg) = prog.segment_at_mut(msg.call_addr) {
            seg.inline_comments
                .insert(msg.call_addr, formatted_comment.clone());
        }
        prog.bookmarks.push(GhidraBookmark {
            address: msg.call_addr,
            bookmark_type: "HopperTag".into(),
            category: "CALL_OBJC".into(),
            comment: formatted_comment,
        });
    }

    /// Port of `HopperHybridObjcAnalyzer.buildHopperByteStateForBlock`.
    pub fn verify_byte_state_slice(prog: &GhidraProgram, addr: u64, count: usize) -> Vec<ByteType> {
        let Some(seg) = prog.segment_at(addr) else {
            return Vec::new();
        };
        (0..count)
            .map(|i| seg.get_type_at_address(addr + i as u64))
            .collect()
    }
}
