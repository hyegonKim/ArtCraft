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
        prog.symbol_table
            .insert(ext_addr, format!("-[{} {}]", msg.receiver, msg.selector));
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

// ============================================================================
// REA 6.10.1 (`ReaGhidraBridge.java` + `hopper_bridge.py`) & Ghidra 12.3_DEV Parity
// ============================================================================

pub const HOPPER_SYMBOL_NAME_UTF16_UNITS: usize = 1024;
pub const CALLER_SCAN_ALLOWANCE_BYTES: usize = 512 * 1024;
pub const DEFAULT_BRIDGE_MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub const MINIMUM_BRIDGE_MAX_REQUEST_BYTES: usize = 7464;
pub const MAXIMUM_BRIDGE_MAX_REQUEST_BYTES: usize = 536_870_887;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeInstructionContext {
    pub address: u64,
    pub instruction_start: Option<u64>,
    pub byte_types: Vec<ByteType>,
    pub symbol: Option<String>,
    pub objc_selector: Option<String>,
    pub eol_comment: Option<String>,
    pub outgoing_refs: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadImageSegmentInfo {
    pub name: String,
    pub start_address: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadImageSummary {
    pub program_name: String,
    pub segments: Vec<LoadImageSegmentInfo>,
    pub symbol_count: usize,
    pub objc_stub_count: usize,
    pub reference_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCallTarget {
    pub to_addr: u64,
    pub ref_type: GhidraRefType,
    pub symbol: Option<String>,
    pub selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRenameFailure {
    pub address: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRenameReport {
    pub applied: Vec<(u64, String)>,
    pub rejected: Vec<BatchRenameFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerScanPage {
    pub target_address: u64,
    pub callers: Vec<u64>,
    pub next_cursor: Option<usize>,
    pub budget_bytes: usize,
}

pub fn parse_bridge_max_request_bytes(raw: &str) -> Result<usize, String> {
    if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "REA_BRIDGE_MAX_REQUEST_BYTES must be ASCII digits in {}..={}",
            MINIMUM_BRIDGE_MAX_REQUEST_BYTES, MAXIMUM_BRIDGE_MAX_REQUEST_BYTES
        ));
    }
    let val: usize = raw.parse().map_err(|_| {
        format!(
            "REA_BRIDGE_MAX_REQUEST_BYTES overflow ({}..={})",
            MINIMUM_BRIDGE_MAX_REQUEST_BYTES, MAXIMUM_BRIDGE_MAX_REQUEST_BYTES
        )
    })?;
    if !(MINIMUM_BRIDGE_MAX_REQUEST_BYTES..=MAXIMUM_BRIDGE_MAX_REQUEST_BYTES).contains(&val) {
        return Err(format!(
            "REA_BRIDGE_MAX_REQUEST_BYTES out of range ({}..={})",
            MINIMUM_BRIDGE_MAX_REQUEST_BYTES, MAXIMUM_BRIDGE_MAX_REQUEST_BYTES
        ));
    }
    Ok(val)
}

fn validate_symbol_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("symbol name must not be empty".to_string());
    }
    if trimmed.chars().any(char::is_control) {
        return Err("symbol name must not contain control characters".to_string());
    }
    let utf16_len = trimmed.encode_utf16().count();
    if utf16_len > HOPPER_SYMBOL_NAME_UTF16_UNITS {
        return Err(format!(
            "symbol name UTF-16 length {} exceeds Hopper ceiling {}",
            utf16_len, HOPPER_SYMBOL_NAME_UTF16_UNITS
        ));
    }
    Ok(())
}

impl GhidraProgram {
    pub fn segment_at(&self, addr: u64) -> Option<&Segment> {
        self.segments.iter().find(|s| s.contains(addr))
    }

    pub fn segment_at_mut(&mut self, addr: u64) -> Option<&mut Segment> {
        self.segments.iter_mut().find(|s| s.contains(addr))
    }

    /// REA 6.10.1 `inspect_native_load_image` equivalent.
    pub fn inspect_native_load_image(&self) -> LoadImageSummary {
        LoadImageSummary {
            program_name: self.name.clone(),
            segments: self
                .segments
                .iter()
                .map(|s| LoadImageSegmentInfo {
                    name: s.name.clone(),
                    start_address: s.start_address,
                    length: s.length(),
                })
                .collect(),
            symbol_count: self.symbol_table.len(),
            objc_stub_count: self.objc_stub_selectors.len(),
            reference_count: self.references.len(),
        }
    }

    /// REA 6.10.1 `inspect_native_instruction` equivalent.
    pub fn inspect_native_instruction(&self, addr: u64) -> Option<NativeInstructionContext> {
        let seg = self.segment_at(addr)?;
        let base = seg.get_instruction_start(addr);
        let byte_types = (0..4)
            .filter(|i| seg.contains(base + *i))
            .map(|i| seg.get_type_at_address(base + i))
            .collect();
        let outgoing_refs = self
            .references
            .iter()
            .filter(|r| r.from_addr == base)
            .map(|r| r.to_addr)
            .collect();
        Some(NativeInstructionContext {
            address: addr,
            instruction_start: Some(base),
            byte_types,
            symbol: self.symbol_table.get(&base).cloned(),
            objc_selector: self.objc_stub_selectors.get(&base).cloned(),
            eol_comment: self.eol_comments.get(&base).cloned(),
            outgoing_refs,
        })
    }

    /// REA 6.10.1 `resolve_native_call_targets` equivalent.
    pub fn resolve_native_call_targets(&self, call_addr: u64) -> Vec<ResolvedCallTarget> {
        self.references
            .iter()
            .filter(|r| {
                r.from_addr == call_addr
                    && matches!(
                        r.ref_type,
                        GhidraRefType::CallOverrideUnconditional
                            | GhidraRefType::UnconditionalCall
                            | GhidraRefType::ComputedJump
                    )
            })
            .map(|r| ResolvedCallTarget {
                to_addr: r.to_addr,
                ref_type: r.ref_type,
                symbol: self.symbol_table.get(&r.to_addr).cloned(),
                selector: self.objc_stub_selectors.get(&r.to_addr).cloned(),
            })
            .collect()
    }

    /// REA 6.10.1 `annotate_native_function` equivalent.
    pub fn annotate_native_function(
        &mut self,
        addr: u64,
        name: Option<&str>,
        comment: Option<&str>,
    ) -> Result<(), String> {
        if let Some(n) = name {
            validate_symbol_name(n)?;
            let clean = n.trim().to_string();
            self.symbol_table.insert(addr, clean.clone());
            if let Some(seg) = self.segment_at_mut(addr) {
                seg.set_name_at_address(addr, clean);
            }
        }
        if let Some(c) = comment {
            let clean = c.trim().to_string();
            self.eol_comments.insert(addr, clean.clone());
            if let Some(seg) = self.segment_at_mut(addr) {
                seg.inline_comments.insert(addr, clean.clone());
            }
            self.bookmarks.push(GhidraBookmark {
                address: addr,
                bookmark_type: "HopperTag".into(),
                category: "REAAnnotation".into(),
                comment: clean,
            });
        }
        Ok(())
    }

    /// REA `search_procedures` with secondary `objc_stub::<sel>` alias matching.
    pub fn search_procedures(&self, query: &str, limit: usize) -> Vec<(u64, String)> {
        let q = query.trim().to_ascii_lowercase();
        let mut results = Vec::new();
        for (&addr, sym) in &self.symbol_table {
            let stub_alias = self
                .objc_stub_selectors
                .get(&addr)
                .map(|s| format!("objc_stub::{s}"));
            let matched = q.is_empty()
                || sym.to_ascii_lowercase().contains(&q)
                || stub_alias
                    .as_ref()
                    .is_some_and(|a| a.to_ascii_lowercase().contains(&q));
            if matched {
                results.push((addr, sym.clone()));
                if results.len() >= limit.max(1) {
                    break;
                }
            }
        }
        results
    }

    /// REA `set_addresses_names` batch symbol rename with per-address validation.
    pub fn apply_address_names(&mut self, items: &[(u64, String)]) -> BatchRenameReport {
        let mut applied = Vec::new();
        let mut rejected = Vec::new();
        for (addr, name) in items {
            match validate_symbol_name(name) {
                Ok(()) => {
                    let clean = name.trim().to_string();
                    self.symbol_table.insert(*addr, clean.clone());
                    if let Some(seg) = self.segment_at_mut(*addr) {
                        seg.set_name_at_address(*addr, clean.clone());
                    }
                    applied.push((*addr, clean));
                }
                Err(reason) => rejected.push(BatchRenameFailure {
                    address: *addr,
                    reason,
                }),
            }
        }
        BatchRenameReport { applied, rejected }
    }

    /// REA `hopper_bridge.py` bounded `CALLER_SCAN_ALLOWANCE_BYTES` caller-scan page.
    pub fn caller_scan_page(&self, target_addr: u64, cursor: usize) -> CallerScanPage {
        let mut all_callers: Vec<u64> = self
            .references
            .iter()
            .filter(|r| r.to_addr == target_addr)
            .map(|r| r.from_addr)
            .collect();
        all_callers.sort_unstable();
        all_callers.dedup();

        let mut callers = Vec::new();
        let mut used_bytes = 64usize;
        let mut idx = cursor.min(all_callers.len());
        while idx < all_callers.len() {
            let entry_cost = 24usize;
            if !callers.is_empty() && used_bytes + entry_cost > CALLER_SCAN_ALLOWANCE_BYTES {
                break;
            }
            callers.push(all_callers[idx]);
            used_bytes += entry_cost;
            idx += 1;
        }
        let next_cursor = (idx < all_callers.len()).then_some(idx);
        CallerScanPage {
            target_address: target_addr,
            callers,
            next_cursor,
            budget_bytes: CALLER_SCAN_ALLOWANCE_BYTES,
        }
    }
}

#[cfg(test)]
mod rea_6_10_parity_tests {
    use super::*;

    #[test]
    fn test_rea_6_10_bridge_ceiling_and_native_tools() {
        assert_eq!(parse_bridge_max_request_bytes("7464"), Ok(7464));
        assert_eq!(parse_bridge_max_request_bytes("16777216"), Ok(16777216));
        assert!(parse_bridge_max_request_bytes("7463").is_err());
        assert!(parse_bridge_max_request_bytes("07464").is_err());
        assert!(parse_bridge_max_request_bytes("536870888").is_err());

        let mut seg = Segment::new("__TEXT", 0x100000000, 0, vec![0u8; 64]);
        seg.mark_as_procedure(0x100000000, 4);
        let mut prog = GhidraProgram::new("Rea610Demo", seg);
        prog.objc_stub_selectors
            .insert(0x100000020, "initWithUTF8String:".into());
        prog.symbol_table
            .insert(0x100000020, "objc_msgSend$initWithUTF8String:".into());
        prog.add_memory_reference(
            0x10000000c,
            0x100000020,
            GhidraRefType::CallOverrideUnconditional,
            GhidraSourceType::Analysis,
            0,
        );

        let img = prog.inspect_native_load_image();
        assert_eq!(img.segments.len(), 1);
        assert_eq!(img.objc_stub_count, 1);

        let targets = prog.resolve_native_call_targets(0x10000000c);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].selector.as_deref(), Some("initWithUTF8String:"));

        let hits = prog.search_procedures("objc_stub::initWithUTF8String:", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 0x100000020);

        prog.annotate_native_function(
            0x100000000,
            Some("bootstrap_entry"),
            Some("REA 6.10.1 verified entry"),
        )
        .unwrap();
        let ctx = prog.inspect_native_instruction(0x100000002).unwrap();
        assert_eq!(ctx.instruction_start, Some(0x100000000));
        assert_eq!(ctx.symbol.as_deref(), Some("bootstrap_entry"));

        let overlong = "x".repeat(HOPPER_SYMBOL_NAME_UTF16_UNITS + 1);
        let report = prog.apply_address_names(&[
            (0x100000010, "valid_helper".into()),
            (0x100000014, overlong),
        ]);
        assert_eq!(report.applied.len(), 1);
        assert_eq!(report.rejected.len(), 1);

        let page = prog.caller_scan_page(0x100000020, 0);
        assert_eq!(page.callers, vec![0x10000000c]);
        assert_eq!(page.next_cursor, None);
    }
}
