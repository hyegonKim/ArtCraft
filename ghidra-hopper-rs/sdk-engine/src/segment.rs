//! Hopper-inspired Byte-Granular Reactive State Machine fused with Ghidra's AddressSpace.
//!
//! Based on REA Evidence `ev_aa86da22f74d2a998f662df1ca367499bd49c9d461a5542dde1b6c8aca50fb1e`
//! (`Contents/Resources/hopper_api.py`), Hopper tracks every byte in a segment with an explicit
//! `TYPE_*` state (`TYPE_UNDEFINED`, `TYPE_CODE`, `TYPE_NEXT`, `TYPE_PROCEDURE`, etc.) and
//! per-operand display formatting (`FORMAT_*`), allowing O(1) interactive re-typing and patching.

use std::collections::{BTreeMap, BTreeSet};

pub const BAD_ADDRESS: u64 = u64::MAX;

/// Hopper `Segment.TYPE_*` byte state classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ByteType {
    Undefined = 0,
    Outside = 1,
    Next = 2,
    Int8 = 3,
    Int16 = 4,
    Int32 = 5,
    Int64 = 6,
    Ascii = 7,
    Unicode = 8,
    Align = 63,
    Code = 65,
    Procedure = 66,
    Structure = 67,
}

impl ByteType {
    pub fn as_str(self) -> &'static str {
        match self {
            ByteType::Undefined => "undefined",
            ByteType::Outside => "outside",
            ByteType::Next => "next",
            ByteType::Int8 => "int8",
            ByteType::Int16 => "int16",
            ByteType::Int32 => "int32",
            ByteType::Int64 => "int64",
            ByteType::Ascii => "ascii",
            ByteType::Unicode => "unicode",
            ByteType::Align => "align",
            ByteType::Code => "code",
            ByteType::Procedure => "procedure",
            ByteType::Structure => "structure",
        }
    }
}

/// Hopper `Document.FORMAT_*` operand formatting flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandFormat {
    pub base: OperandFormatKind,
    pub negate: bool,
    pub leading_zeroes: bool,
    pub signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OperandFormatKind {
    Default = 0,
    Hexadecimal = 1,
    Decimal = 2,
    Octal = 3,
    Character = 4,
    StackVariable = 5,
    Offset = 6,
    Address = 7,
    Float = 8,
    Binary = 9,
    Structured = 10,
    Enum = 11,
    AddressDiff = 12,
}

impl Default for OperandFormat {
    fn default() -> Self {
        Self {
            base: OperandFormatKind::Default,
            negate: false,
            leading_zeroes: false,
            signed: false,
        }
    }
}

impl OperandFormat {
    pub fn format_imm(&self, raw_val: u64) -> String {
        let mut val = raw_val;
        let prefix = if self.negate {
            val = raw_val.wrapping_neg();
            "-"
        } else {
            ""
        };
        match self.base {
            OperandFormatKind::Default | OperandFormatKind::Hexadecimal => {
                if self.signed && (val as i64) < 0 {
                    format!("-0x{:x}", (val as i64).wrapping_neg() as u64)
                } else if self.leading_zeroes {
                    format!("{}0x{:016x}", prefix, val)
                } else {
                    format!("{}0x{:x}", prefix, val)
                }
            }
            OperandFormatKind::Decimal => {
                if self.signed {
                    format!("{}{}", prefix, val as i64)
                } else {
                    format!("{}{}", prefix, val)
                }
            }
            OperandFormatKind::Octal => format!("{}0o{:o}", prefix, val),
            OperandFormatKind::Binary => format!("{}0b{:b}", prefix, val),
            OperandFormatKind::Character => {
                let c = (val as u8) as char;
                if c.is_ascii_graphic() || c == ' ' {
                    format!("'{}'", c)
                } else {
                    format!("0x{:02x}", val as u8)
                }
            }
            OperandFormatKind::Address => format!("sub_{:x}", val),
            OperandFormatKind::StackVariable => format!("var_{:x}", val),
            _ => format!("0x{:x}", val),
        }
    }
}

/// Call reference classification combining Ghidra XREFs with Hopper's `CALL_OBJC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallType {
    None = 0,
    Unknown = 1,
    Direct = 2,
    ObjC = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallReference {
    pub call_type: CallType,
    pub from_address: u64,
    pub to_address: u64,
    pub selector_hint: Option<String>,
}

/// Hopper-style Tag & Color annotation applicable to Addresses, BasicBlocks, or Procedures.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag {
    pub name: String,
    pub color_hex: String,
}

impl Tag {
    pub fn new(name: impl Into<String>, color_hex: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color_hex: color_hex.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub start_address: u64,
    pub length: u64,
    pub flags: u32,
}

/// Memory Segment combining Ghidra's MemoryBlock with Hopper's flat O(1) `Vec<ByteType>`.
#[derive(Debug, Clone)]
pub struct Segment {
    pub function_starts: BTreeSet<u64>,
    pub name: String,
    /// Loader permissions, normalized to read=1, write=2, execute=4.
    /// Synthetic segments default to unknown.
    pub permissions: Option<u8>,
    pub start_address: u64,
    pub file_offset: u64,
    pub data: Vec<u8>,
    pub byte_types: Vec<ByteType>,
    pub sections: Vec<Section>,
    pub labels: BTreeMap<u64, String>,
    pub demangled_labels: BTreeMap<u64, String>,
    pub prefix_comments: BTreeMap<u64, String>,
    pub inline_comments: BTreeMap<u64, String>,
    pub refs_from: BTreeMap<u64, BTreeSet<u64>>,
    pub refs_to: BTreeMap<u64, BTreeSet<u64>>,
    pub operand_formats: BTreeMap<(u64, usize), OperandFormat>,
    pub address_tags: BTreeMap<u64, BTreeSet<Tag>>,
    pub thumb_mode: BTreeSet<u64>,
}

impl Segment {
    pub fn new(
        name: impl Into<String>,
        start_address: u64,
        file_offset: u64,
        bytes: Vec<u8>,
    ) -> Self {
        let len = bytes.len();
        Self {
            function_starts: BTreeSet::new(),
            name: name.into(),
            permissions: None,
            start_address,
            file_offset,
            data: bytes,
            byte_types: vec![ByteType::Undefined; len],
            sections: Vec::new(),
            labels: BTreeMap::new(),
            demangled_labels: BTreeMap::new(),
            prefix_comments: BTreeMap::new(),
            inline_comments: BTreeMap::new(),
            refs_from: BTreeMap::new(),
            refs_to: BTreeMap::new(),
            operand_formats: BTreeMap::new(),
            address_tags: BTreeMap::new(),
            thumb_mode: BTreeSet::new(),
        }
    }

    #[inline]
    pub fn length(&self) -> u64 {
        self.data.len() as u64
    }

    #[inline]
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start_address && (addr - self.start_address) < self.length()
    }

    /// A declared entry or section boundary requires an explicit transfer;
    /// ordinary fall-through is uncertain, not an invented tail call.
    pub fn allows_fallthrough(&self, from: u64, to: u64) -> bool {
        if self.function_starts.contains(&to) {
            return false;
        }
        self.sections
            .iter()
            .filter(|s| {
                from >= s.start_address
                    && s.start_address
                        .checked_add(s.length)
                        .is_some_and(|end| from.checked_add(4).is_some_and(|last| last <= end))
            })
            .all(|s| {
                to >= s.start_address
                    && s.start_address
                        .checked_add(s.length)
                        .is_some_and(|end| to.checked_add(4).is_some_and(|last| last <= end))
            })
    }

    #[inline]
    fn offset_of(&self, addr: u64) -> Option<usize> {
        if self.contains(addr) {
            Some((addr - self.start_address) as usize)
        } else {
            None
        }
    }

    pub fn file_offset_for_address(&self, addr: u64) -> Option<u64> {
        self.offset_of(addr)
            .and_then(|off| self.file_offset.checked_add(off as u64))
    }

    pub fn get_type_at_address(&self, addr: u64) -> ByteType {
        match self.offset_of(addr) {
            Some(off) => self.byte_types[off],
            None => ByteType::Outside,
        }
    }

    /// Set a multi-byte object at `addr` with head type `head_type` and trailing `ByteType::Next`.
    pub fn set_type_at_address(&mut self, addr: u64, length: usize, head_type: ByteType) -> bool {
        let Some(off) = self.offset_of(addr) else {
            return false;
        };
        if length == 0 || length > self.byte_types.len() - off {
            return false;
        }
        self.byte_types[off] = head_type;
        for b in &mut self.byte_types[off + 1..off + length] {
            *b = ByteType::Next;
        }
        true
    }

    pub fn mark_as_code(&mut self, addr: u64, instr_len: usize) -> bool {
        self.set_type_at_address(addr, instr_len, ByteType::Code)
    }

    pub fn mark_as_procedure(&mut self, addr: u64, first_instr_len: usize) -> bool {
        self.set_type_at_address(addr, first_instr_len, ByteType::Procedure)
    }

    pub fn mark_range_as_undefined(&mut self, addr: u64, length: usize) -> bool {
        let Some(off) = self.offset_of(addr) else {
            return false;
        };
        let end = off.saturating_add(length).min(self.byte_types.len());
        for b in &mut self.byte_types[off..end] {
            *b = ByteType::Undefined;
        }
        true
    }

    pub fn make_alignment(&mut self, addr: u64, alignment: u64) -> Option<u64> {
        if !matches!(alignment, 2 | 4 | 8 | 16 | 32 | 64) {
            return None;
        }
        let rem = addr % alignment;
        let pad = if rem == 0 {
            0
        } else {
            (alignment - rem) as usize
        };
        if pad > 0 && !self.set_type_at_address(addr, pad, ByteType::Align) {
            return None;
        }
        addr.checked_add(pad as u64)
    }

    /// Hopper's `nearestBlock` / `getInstructionStart`: walks backward across `TYPE_NEXT` bytes.
    pub fn get_instruction_start(&self, addr: u64) -> u64 {
        let Some(mut off) = self.offset_of(addr) else {
            return BAD_ADDRESS;
        };
        while off > 0 && self.byte_types[off] == ByteType::Next {
            off -= 1;
        }
        self.start_address + off as u64
    }

    /// Hopper's `getObjectLength`: counts the head byte plus contiguous `TYPE_NEXT` bytes.
    pub fn get_object_length(&self, addr: u64) -> usize {
        let Some(off) = self.offset_of(addr) else {
            return 0;
        };
        let mut len = 1;
        while off + len < self.byte_types.len() && self.byte_types[off + len] == ByteType::Next {
            len += 1;
        }
        len
    }

    pub fn get_next_address_with_type(&self, start_addr: u64, target: ByteType) -> u64 {
        let Some(start_off) = self.offset_of(start_addr) else {
            return BAD_ADDRESS;
        };
        for (idx, &t) in self.byte_types[start_off..].iter().enumerate() {
            if t == target {
                return start_addr + idx as u64;
            }
        }
        BAD_ADDRESS
    }

    pub fn read_bytes(&self, addr: u64, len: usize) -> Option<&[u8]> {
        let off = self.offset_of(addr)?;
        self.data.get(off..off.checked_add(len)?)
    }

    pub fn read_u32_le(&self, addr: u64) -> Option<u32> {
        let b = self.read_bytes(addr, 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_u64_le(&self, addr: u64) -> Option<u64> {
        let b = self.read_bytes(addr, 8)?;
        Some(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Live instruction/byte patching (Hopper `writeBytes` + `Assembler.xpc` style).
    pub fn write_bytes(&mut self, addr: u64, patch: &[u8]) -> bool {
        let Some(off) = self.offset_of(addr) else {
            return false;
        };
        if off + patch.len() > self.data.len() {
            return false;
        }
        self.data[off..off + patch.len()].copy_from_slice(patch);
        true
    }

    pub fn add_reference(&mut self, from_addr: u64, to_addr: u64) {
        self.refs_from.entry(from_addr).or_default().insert(to_addr);
        self.refs_to.entry(to_addr).or_default().insert(from_addr);
    }

    pub fn set_name_at_address(&mut self, addr: u64, name: impl Into<String>) {
        let raw = name.into();
        let demangled = demangle_symbol(&raw);
        if demangled != raw {
            self.demangled_labels.insert(addr, demangled);
        }
        self.labels.insert(addr, raw);
    }

    pub fn get_name_at_address(&self, addr: u64) -> Option<&str> {
        self.labels.get(&addr).map(String::as_str)
    }

    pub fn get_demangled_name_at_address(&self, addr: u64) -> Option<&str> {
        self.demangled_labels
            .get(&addr)
            .or_else(|| self.labels.get(&addr))
            .map(String::as_str)
    }

    /// Auto-discover ASCII strings inside the segment and mark them as `ByteType::Ascii`.
    pub fn scan_ascii_strings(&mut self, min_len: usize) -> Vec<(u64, String)> {
        let mut found = Vec::new();
        let mut i = 0;
        while i < self.data.len() {
            if self.byte_types[i] == ByteType::Undefined && is_printable_ascii(self.data[i]) {
                let start = i;
                while i < self.data.len() && is_printable_ascii(self.data[i]) {
                    i += 1;
                }
                if i < self.data.len() && self.data[i] == 0 && (i - start) >= min_len {
                    let s = String::from_utf8_lossy(&self.data[start..i]).to_string();
                    let addr = self.start_address + start as u64;
                    self.set_type_at_address(addr, (i - start) + 1, ByteType::Ascii);
                    found.push((addr, s));
                    i += 1;
                    continue;
                }
            }
            i += 1;
        }
        found
    }

    /// Hopper Interactive Hotkey State Machine (`C` = Code, `P` = Procedure, `A` = Ascii,
    /// `D` = Int64 Data, `U` = Undefine entire object from its head byte).
    pub fn apply_hopper_hotkey(&mut self, addr: u64, hotkey: char) -> bool {
        match hotkey.to_ascii_uppercase() {
            'C' => self.mark_as_code(addr, 4),
            'P' => self.mark_as_procedure(addr, 4),
            'D' => self.set_type_at_address(addr, 8, ByteType::Int64),
            'A' => {
                let Some(off) = self.offset_of(addr) else {
                    return false;
                };
                let mut len = 0usize;
                while off + len < self.data.len() && is_printable_ascii(self.data[off + len]) {
                    len += 1;
                }
                if len == 0 {
                    return false;
                }
                let total = if off + len < self.data.len() && self.data[off + len] == 0 {
                    len + 1
                } else {
                    len
                };
                self.set_type_at_address(addr, total, ByteType::Ascii)
            }
            'U' => {
                let head = self.get_instruction_start(addr);
                if head == BAD_ADDRESS {
                    return false;
                }
                let obj_len = self.get_object_length(head);
                self.mark_range_as_undefined(head, obj_len)
            }
            _ => false,
        }
    }
}

fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7e).contains(&b)
}

/// Hopper-style Swift 5 (`_$s` / `$s`), C++ (`__Z`), and Objective-C (`-[Class sel:]`) symbol demangler.
/// Parses length-prefixed Swift 5 identifiers into `Module.Type.method(...)` notation.
pub fn demangle_symbol(sym: &str) -> String {
    if let Some(rest) = sym.strip_prefix("_$s").or_else(|| sym.strip_prefix("$s")) {
        let mut parts = Vec::new();
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_digit() {
                let mut num = 0usize;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    let Some(next) = num
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((bytes[i] - b'0') as usize))
                    else {
                        return format!("swift::{}", rest);
                    };
                    num = next;
                    i += 1;
                }
                if num > 0 && num <= bytes.len() - i {
                    parts.push(String::from_utf8_lossy(&bytes[i..i + num]).to_string());
                    i += num;
                    continue;
                }
            }
            i += 1;
        }
        if !parts.is_empty() {
            return format!("swift::{}", parts.join("."));
        }
        return format!("swift::{}", rest);
    }
    if let Some(rest) = sym.strip_prefix("__Z").or_else(|| sym.strip_prefix("_Z")) {
        return format!("cxx::{}", rest);
    }
    if sym.starts_with("-[") || sym.starts_with("+[") {
        return sym.to_string();
    }
    sym.strip_prefix('_').unwrap_or(sym).to_string()
}
