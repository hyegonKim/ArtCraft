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
            if let Some(seg) = prog.text_segment_mut() {
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
    next_external_addr: u64,
}

impl GhidraProgram {
    pub fn new(name: impl Into<String>, text_seg: Segment) -> Self {
        Self {
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
            next_external_addr: 0x2000_0000_0000,
        }
    }

    pub fn from_macho_image(name: impl Into<String>, img: &MachOImage) -> Self {
        let mut prog = Self {
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
            next_external_addr: 0x2000_0000_0000,
        };
        for (&stub_addr, sel) in &prog.objc_stub_selectors {
            prog.symbol_table
                .insert(stub_addr, format!("objc_msgSend${}", sel));
        }
        prog
    }

    pub fn from_pe_image(name: impl Into<String>, img: &crate::pe_x64::Pe64Image) -> Self {
        let mut prog = Self {
            name: name.into(),
            segments: img.segments.clone(),
            symbol_table: img.exports.clone(),
            external_methods: BTreeMap::new(),
            references: Vec::new(),
            eol_comments: BTreeMap::new(),
            bookmarks: Vec::new(),
            objc_stub_selectors: BTreeMap::new(),
            switch_hints: SwitchHintTable::default(),
            data_types: ObjcDataTypes::default(),
            next_external_addr: 0x2000_0000_0000,
        };
        // Every `.pdata` function gets a symbol; only the first 8 also get a bookmark.
        for (idx, rf) in img.runtime_functions.iter().enumerate() {
            let fn_va = img.va(rf.begin_rva);
            prog.symbol_table
                .entry(fn_va)
                .or_insert_with(|| format!("FUN_{:x}", fn_va));
            if idx < 8 {
                let end = rf
                    .end_rva
                    .map_or_else(|| "?".to_string(), |end| format!("0x{end:x}"));
                prog.bookmarks.push(GhidraBookmark {
                    address: fn_va,
                    bookmark_type: "HopperTag".into(),
                    category: "PE64RuntimeFunction".into(),
                    comment: format!(
                        ".pdata procedure [0x{:x}..{}] unwind=0x{:x}",
                        rf.begin_rva, end, rf.unwind_data
                    ),
                });
            }
        }
        prog
    }

    pub fn text_segment(&self) -> Option<&Segment> {
        self.segments.first()
    }

    pub fn text_segment_mut(&mut self) -> Option<&mut Segment> {
        self.segments.first_mut()
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
        let stubs = prog.objc_stub_selectors.clone();
        let hints = prog.switch_hints.clone();
        let mut proc = {
            let seg = prog
                .text_segment_mut()
                .expect("GhidraProgram must have a __TEXT segment");
            Procedure::analyze_arm64(func_name, entry_addr, max_insns, seg, &stubs, &hints)
        };

        // 3. Scan lifted P-Code ops in-memory (Zero-IPC!) to resolve ObjcMessage calls
        //    (replaces Ghidra's slow ParallelDecompiler IPC callback in ObjcMessageAnalyzer.java)
        let mut reg_strings: BTreeMap<u64, String> = BTreeMap::new();
        let mut reg_consts: BTreeMap<u64, u64> = BTreeMap::new();
        let mut pending_messages = Vec::new();

        if let Some(seg) = prog.text_segment() {
            for bb in &proc.basic_blocks {
                for insn in &bb.instructions {
                    for op in &insn.pcode {
                        match op {
                            crate::pcode_sleigh::PcodeOp::Copy { dst, src } => {
                                if dst.space == crate::pcode_sleigh::SpaceKind::Register
                                    && src.space == crate::pcode_sleigh::SpaceKind::Constant
                                {
                                    reg_consts.insert(dst.offset, src.offset);
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::IntAdd { dst, lhs, rhs } => {
                                if let Some(&base) = reg_consts.get(&lhs.offset) {
                                    let target = base.wrapping_add(rhs.offset);
                                    reg_consts.insert(dst.offset, target);
                                    if let Some(bytes) = seg.read_bytes(target, 64) {
                                        if let Some(end) = bytes.iter().position(|&b| b == 0) {
                                            if end > 0 {
                                                let s =
                                                    String::from_utf8_lossy(&bytes[..end]).into();
                                                reg_strings.insert(dst.offset, s);
                                            }
                                        }
                                    }
                                }
                            }
                            crate::pcode_sleigh::PcodeOp::CallObjC {
                                stub_addr,
                                selector,
                            } => {
                                let receiver = reg_strings
                                    .get(&0)
                                    .cloned()
                                    .unwrap_or_else(|| "HopperGhidraBridge".to_string());
                                let arg = reg_strings
                                    .get(&2)
                                    .map(|s| format!("@\"{}\"", s))
                                    .unwrap_or_else(|| "x2".to_string());
                                pending_messages.push(ObjcMessage {
                                    receiver,
                                    selector: selector.clone(),
                                    function_entry: entry_addr,
                                    call_addr: insn.address,
                                    stub_target: *stub_addr,
                                    parameters: vec![arg],
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // 4. Port of `updateExternalBlock` + `setCommentAndReference` in `ObjcMessageAnalyzer.java`
        for msg in pending_messages {
            self.update_external_block(prog, &msg);
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

    /// Port of `ObjcMessageAnalyzer.updateExternalBlock(Program, Message, MessageLog)`.
    fn update_external_block(&self, prog: &mut GhidraProgram, msg: &ObjcMessage) -> u64 {
        let key = (msg.receiver.clone(), msg.selector.clone());
        if let Some(&existing) = prog.external_methods.get(&key) {
            return existing;
        }
        let ext_addr = prog.next_external_addr;
        prog.next_external_addr += prog.data_types.ptr_size as u64;
        prog.external_methods.insert(key, ext_addr);
        prog.symbol_table.insert(
            ext_addr,
            format!("-[{} {}]", msg.receiver, msg.selector),
        );
        ext_addr
    }

    /// Port of `ObjcMessageAnalyzer.setCommentAndReference(Program, Message, List<String>, MessageLog)`.
    fn set_comment_and_reference(&self, prog: &mut GhidraProgram, msg: &ObjcMessage) {
        let key = (msg.receiver.clone(), msg.selector.clone());
        let target_addr = prog
            .external_methods
            .get(&key)
            .copied()
            .unwrap_or(msg.stub_target);

        let ref_type = if self.use_call_overrides {
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
        if let Some(seg) = prog.text_segment_mut() {
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
        let Some(seg) = prog.text_segment() else {
            return Vec::new();
        };
        (0..count)
            .map(|i| seg.get_type_at_address(addr + i as u64))
            .collect()
    }
}
