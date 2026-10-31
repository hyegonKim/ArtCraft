//! Windows PE32+ (64-bit) image parser in pure Rust, with no external dependencies.
//!
//! Reads the headers and tables laid out in Microsoft's PE/COFF specification:
//! - DOS header `MZ` (`0x5A4D`), `e_lfanew` at `0x3C`, `PE\0\0` signature (`0x00004550`)
//! - COFF file header and the PE32+ optional header (magic `0x020B`) with its data directories
//! - Section headers; each section's raw data (at most [`MAX_SEGMENT_BYTES`]) is copied into a
//!   [`Segment`]
//! - Export directory (named exports and forwarders) and import directory (by name and ordinal)
//! - Exception directory (`.pdata`): 12-byte x64 `RUNTIME_FUNCTION` entries or 8-byte ARM64
//!   entries
//! - ASCII and UTF-16LE string scanning
//!
//! There is no instruction decoder. Procedure starts come from `.pdata` and the entry point, and
//! since instruction lengths are unknown only the head byte of each one is marked
//! `ByteType::Procedure`. Offsets taken from the file use checked arithmetic (or are bounded by
//! an already-checked offset) and every read is bounds-checked, so malformed input yields an
//! error or a note, never a panic.

use crate::segment::{Section, Segment};
use std::collections::{BTreeMap, BTreeSet};

pub const MZ_MAGIC: u16 = 0x5A4D;
pub const PE_MAGIC: u32 = 0x0000_4550;
pub const PE32_PLUS_MAGIC: u16 = 0x020B;
pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
pub const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

/// Each segment keeps at most this much of its section's raw data.
pub const MAX_SEGMENT_BYTES: usize = 2 * 1024 * 1024;

const COFF_HEADER_SIZE: usize = 20;
/// The PE32+ optional header up to and including `NumberOfRvaAndSizes`.
const PE32_PLUS_FIXED_OPTIONAL_HEADER: usize = 112;
const DATA_DIRECTORY_SIZE: usize = 8;
const MAX_DATA_DIRECTORIES: usize = 16;
const DIR_EXPORT: usize = 0;
const DIR_IMPORT: usize = 1;
const DIR_EXCEPTION: usize = 3;
const SECTION_HEADER_SIZE: usize = 40;
/// The Windows loader accepts at most 96 sections.
const MAX_SECTIONS: usize = 96;
const IMPORT_DESCRIPTOR_SIZE: usize = 20;
const THUNK_SIZE: usize = 8;
/// Upper bound on import thunks read, against descriptors that share huge thunk arrays.
const MAX_IMPORTED_FUNCTIONS: usize = 1 << 20;
const X64_RUNTIME_FUNCTION_SIZE: usize = 12;
const ARM64_RUNTIME_FUNCTION_SIZE: usize = 8;
const MAX_SYMBOL_NAME: usize = 256;
const MAX_STRING_SCAN_BYTES: usize = 4 * 1024 * 1024;
const MAX_NOTABLE_STRINGS: usize = 120;
const MIN_SCAN_LEN: usize = 5;
const MAX_SCAN_LEN: usize = 200;

const ERR_DOS: &str = "Truncated DOS header";
const ERR_LFANEW: &str = "e_lfanew points outside the file";
const ERR_COFF: &str = "Truncated COFF header";
const ERR_OPTIONAL: &str = "Truncated optional header";
const ERR_OVERFLOW: &str = "Header offset overflows";

#[derive(Debug, Clone)]
pub struct PeSectionInfo {
    pub name: String,
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_data_ptr: u32,
    pub raw_data_size: u32,
    pub characteristics: u32,
}

/// One `.pdata` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFunctionEntry {
    pub begin_rva: u32,
    /// `None` when an ARM64 function length could not be read.
    pub end_rva: Option<u32>,
    /// x64: RVA of the `UNWIND_INFO`. ARM64: the raw unwind word (packed unwind data, or the RVA
    /// of an `.xdata` record when its low two bits are 0).
    pub unwind_data: u32,
}

#[derive(Debug, Clone)]
pub struct Pe64Image {
    pub machine: u16,
    pub arch_name: &'static str,
    pub image_base: u64,
    pub entry_point_rva: u32,
    pub sections: Vec<PeSectionInfo>,
    pub segments: Vec<Segment>,
    /// Named, non-forwarded exports by address; the first name wins when several share one.
    pub exports: BTreeMap<u64, String>,
    /// Named, non-forwarded exports read (can exceed `exports.len()`).
    pub named_export_count: usize,
    /// Forwarded exports: `(name, forwarder)`, e.g. `("HeapAlloc", "NTDLL.RtlAllocateHeap")`.
    pub forwarded_exports: Vec<(String, String)>,
    pub imported_dlls: Vec<String>,
    /// `(dll, function)`; imports by ordinal are named `#<ordinal>`.
    pub imported_functions: Vec<(String, String)>,
    pub runtime_functions: Vec<RuntimeFunctionEntry>,
    /// Procedure head bytes marked in `segments`: the entry point and `.pdata` starts that fall
    /// inside a segment's loaded bytes.
    pub procedures_marked: usize,
    pub notable_strings: Vec<String>,
    /// Parts that were skipped or cut short, e.g. `.pdata` of an unsupported machine.
    pub notes: Vec<String>,
}

#[derive(Debug, Default)]
struct Exports {
    by_address: BTreeMap<u64, String>,
    named: usize,
    forwarded: Vec<(String, String)>,
}

impl Pe64Image {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mz = read_u16_at(bytes, 0, 0).ok_or(ERR_DOS)?;
        if mz != MZ_MAGIC {
            return Err(format!("Expected MZ magic (0x5A4D), got 0x{mz:04x}"));
        }
        let e_lfanew = read_u32_at(bytes, 0, 0x3c).ok_or(ERR_DOS)?;
        let pe_off = usize::try_from(e_lfanew).map_err(|_| ERR_OVERFLOW)?;
        let pe_sig = read_u32_at(bytes, pe_off, 0).ok_or(ERR_LFANEW)?;
        if pe_sig != PE_MAGIC {
            return Err(format!("Expected PE\\0\\0 signature, got 0x{pe_sig:08x}"));
        }

        let coff_off = pe_off.checked_add(4).ok_or(ERR_OVERFLOW)?;
        let machine = read_u16_at(bytes, coff_off, 0).ok_or(ERR_COFF)?;
        let num_sections = read_u16_at(bytes, coff_off, 2).ok_or(ERR_COFF)?;
        let opt_hdr_size = read_u16_at(bytes, coff_off, 16).ok_or(ERR_COFF)?;
        let opt_hdr_size = usize::from(opt_hdr_size);
        let arch_name = match machine {
            IMAGE_FILE_MACHINE_AMD64 => "x86_64",
            IMAGE_FILE_MACHINE_ARM64 => "arm64",
            _ => "unknown",
        };

        let opt_off = coff_off.checked_add(COFF_HEADER_SIZE).ok_or(ERR_OVERFLOW)?;
        let opt_magic = read_u16_at(bytes, opt_off, 0).ok_or(ERR_OPTIONAL)?;
        if opt_magic != PE32_PLUS_MAGIC {
            return Err(format!(
                "Expected PE32+ magic (0x020B), got 0x{opt_magic:04x}"
            ));
        }
        if opt_hdr_size < PE32_PLUS_FIXED_OPTIONAL_HEADER {
            return Err(format!(
                "SizeOfOptionalHeader {opt_hdr_size} is smaller than the \
                 {PE32_PLUS_FIXED_OPTIONAL_HEADER}-byte PE32+ fixed part"
            ));
        }
        let sec_table_off = opt_off.checked_add(opt_hdr_size).ok_or(ERR_OVERFLOW)?;
        if sec_table_off > bytes.len() {
            return Err(ERR_OPTIONAL.into());
        }

        let entry_point_rva = read_u32_at(bytes, opt_off, 16).ok_or(ERR_OPTIONAL)?;
        let image_base = read_u64_at(bytes, opt_off, 24).ok_or(ERR_OPTIONAL)?;
        if image_base.checked_add(u64::from(u32::MAX)).is_none() {
            return Err(format!(
                "ImageBase 0x{image_base:x} leaves no room for 32-bit RVAs"
            ));
        }
        let num_rva_and_sizes = read_u32_at(bytes, opt_off, 108).ok_or(ERR_OPTIONAL)?;
        let dirs_in_header = (opt_hdr_size - PE32_PLUS_FIXED_OPTIONAL_HEADER) / DATA_DIRECTORY_SIZE;
        let num_dirs = usize::try_from(num_rva_and_sizes)
            .unwrap_or(usize::MAX)
            .min(MAX_DATA_DIRECTORIES)
            .min(dirs_in_header);
        let data_dir = |idx: usize| -> (u32, u32) {
            if idx >= num_dirs {
                return (0, 0);
            }
            let entry = opt_off + PE32_PLUS_FIXED_OPTIONAL_HEADER + idx * DATA_DIRECTORY_SIZE;
            // In bounds: `entry + 8 <= sec_table_off <= bytes.len()`.
            let rva = read_u32_at(bytes, entry, 0).unwrap_or(0);
            let size = read_u32_at(bytes, entry, 4).unwrap_or(0);
            (rva, size)
        };
        let (export_rva, export_size) = data_dir(DIR_EXPORT);
        let (import_rva, _import_size) = data_dir(DIR_IMPORT);
        let (pdata_rva, pdata_size) = data_dir(DIR_EXCEPTION);

        let mut notes = Vec::new();
        let num_sections = usize::from(num_sections);
        let (sections, mut segments) =
            parse_sections(bytes, sec_table_off, num_sections, image_base, &mut notes);
        let exports = parse_exports(bytes, &sections, image_base, export_rva, export_size);
        let (imported_dlls, imported_functions) =
            parse_imports(bytes, &sections, import_rva, &mut notes);
        let pdata = (pdata_rva, pdata_size);
        let runtime_functions =
            parse_runtime_functions(bytes, &sections, machine, pdata, &mut notes);
        let procedures_marked = mark_procedure_starts(
            &mut segments,
            image_base,
            entry_point_rva,
            &runtime_functions,
        );
        let notable_strings = scan_notable_strings(bytes, &sections);

        Ok(Self {
            machine,
            arch_name,
            image_base,
            entry_point_rva,
            sections,
            segments,
            exports: exports.by_address,
            named_export_count: exports.named,
            forwarded_exports: exports.forwarded,
            imported_dlls,
            imported_functions,
            runtime_functions,
            procedures_marked,
            notable_strings,
            notes,
        })
    }

    /// Virtual address of `rva` in this image.
    pub fn va(&self, rva: u32) -> u64 {
        rva_to_va(self.image_base, rva)
    }

    /// File offset of `rva`, or `None` when no section's raw data backs it.
    pub fn rva_to_file_offset(sections: &[PeSectionInfo], rva: u32) -> Option<usize> {
        sections.iter().find_map(|sec| {
            let delta = rva.checked_sub(sec.virtual_address)?;
            let span = sec.virtual_size.max(sec.raw_data_size);
            if delta < span && delta < sec.raw_data_size {
                usize::try_from(sec.raw_data_ptr.checked_add(delta)?).ok()
            } else {
                None
            }
        })
    }

    /// Scans `bytes` for printable ASCII runs and 2-byte-aligned UTF-16LE runs of 5 to 200
    /// characters that contain any of `keywords` (case-insensitive). ASCII matches come first.
    pub fn scan_matching_strings(
        bytes: &[u8],
        keywords: &[&str],
        max_matches: usize,
    ) -> Vec<String> {
        let keywords: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
        let mut results = Vec::new();
        if max_matches == 0 {
            return results;
        }

        for run in bytes.split(|&b| !is_printable_ascii(b)) {
            push_if_match(&String::from_utf8_lossy(run), &keywords, &mut results);
            if results.len() >= max_matches {
                return results;
            }
        }

        let mut run = String::new();
        for pair in bytes.as_chunks::<2>().0 {
            let unit = u16::from_le_bytes(*pair);
            match u8::try_from(unit) {
                Ok(b) if is_printable_ascii(b) => run.push(char::from(b)),
                _ => {
                    push_if_match(&run, &keywords, &mut results);
                    if results.len() >= max_matches {
                        return results;
                    }
                    run.clear();
                }
            }
        }
        push_if_match(&run, &keywords, &mut results);
        results
    }
}

fn parse_sections(
    bytes: &[u8],
    sec_table_off: usize,
    num_sections: usize,
    image_base: u64,
    notes: &mut Vec<String>,
) -> (Vec<PeSectionInfo>, Vec<Segment>) {
    let mut sections = Vec::new();
    let mut segments = Vec::new();
    if num_sections > MAX_SECTIONS {
        notes.push(format!(
            "NumberOfSections is {num_sections}; only the first {MAX_SECTIONS} were read"
        ));
    }
    for i in 0..num_sections.min(MAX_SECTIONS) {
        let header = i
            .checked_mul(SECTION_HEADER_SIZE)
            .and_then(|d| sec_table_off.checked_add(d))
            .and_then(|start| bytes.get(start..start.checked_add(SECTION_HEADER_SIZE)?));
        let Some(header) = header else {
            notes.push(format!(
                "section table truncated after {i} of {num_sections} sections"
            ));
            break;
        };
        let name = clean_fixed_str(&header[..8]);
        let virtual_size = read_u32_at(header, 0, 8).unwrap_or(0);
        let virtual_address = read_u32_at(header, 0, 12).unwrap_or(0);
        let raw_data_size = read_u32_at(header, 0, 16).unwrap_or(0);
        let raw_data_ptr = read_u32_at(header, 0, 20).unwrap_or(0);
        let characteristics = read_u32_at(header, 0, 36).unwrap_or(0);

        let raw_len = usize::try_from(raw_data_size).unwrap_or(usize::MAX);
        if raw_len > MAX_SEGMENT_BYTES {
            notes.push(format!(
                "section {name}: segment holds the first {MAX_SEGMENT_BYTES} of {raw_len} bytes"
            ));
        }
        let keep = raw_len.min(MAX_SEGMENT_BYTES);
        let seg_bytes: Vec<u8> = usize::try_from(raw_data_ptr)
            .ok()
            .and_then(|start| bytes.get(start..))
            .map(|rest| rest.iter().take(keep).copied().collect())
            .unwrap_or_default();

        let vmaddr = rva_to_va(image_base, virtual_address);
        let file_offset = u64::from(raw_data_ptr);
        let mut seg = Segment::new(name.clone(), vmaddr, file_offset, seg_bytes);
        seg.sections.push(Section {
            name: name.clone(),
            start_address: vmaddr,
            length: u64::from(virtual_size),
            flags: characteristics,
        });
        segments.push(seg);
        sections.push(PeSectionInfo {
            name,
            virtual_address,
            virtual_size,
            raw_data_ptr,
            raw_data_size,
            characteristics,
        });
    }
    (sections, segments)
}

fn parse_exports(
    bytes: &[u8],
    sections: &[PeSectionInfo],
    image_base: u64,
    dir_rva: u32,
    dir_size: u32,
) -> Exports {
    let mut out = Exports::default();
    if dir_rva == 0 || dir_size == 0 {
        return out;
    }
    let Some(dir) = Pe64Image::rva_to_file_offset(sections, dir_rva) else {
        return out;
    };
    let to_offset = |rva: u32| Pe64Image::rva_to_file_offset(sections, rva);
    let read_name =
        |rva: u32| to_offset(rva).and_then(|off| read_cstr(bytes, off, MAX_SYMBOL_NAME));
    let table = |delta: usize| read_u32_at(bytes, dir, delta).and_then(to_offset);

    let num_functions = read_u32_at(bytes, dir, 20).unwrap_or(0);
    let num_names = read_u32_at(bytes, dir, 24).unwrap_or(0);
    let (Some(funcs), Some(names), Some(ordinals)) = (table(28), table(32), table(36)) else {
        return out;
    };
    let dir_end = dir_rva.saturating_add(dir_size);

    for i in 0..usize::try_from(num_names).unwrap_or(usize::MAX) {
        let name_rva = i.checked_mul(4).and_then(|d| read_u32_at(bytes, names, d));
        let ordinal = i
            .checked_mul(2)
            .and_then(|d| read_u16_at(bytes, ordinals, d));
        let (Some(name_rva), Some(ordinal)) = (name_rva, ordinal) else {
            break;
        };
        if u32::from(ordinal) >= num_functions {
            continue;
        }
        let func_rva = read_u32_at(bytes, funcs, usize::from(ordinal) * 4);
        let (Some(func_rva), Some(name)) = (func_rva, read_name(name_rva)) else {
            continue;
        };
        if func_rva == 0 {
            continue;
        }
        if (dir_rva..dir_end).contains(&func_rva) {
            // An address inside the export directory is a forwarder string (`DLL.Function`).
            let forwarder = read_name(func_rva).unwrap_or_default();
            out.forwarded.push((name, forwarder));
        } else {
            out.named += 1;
            let addr = rva_to_va(image_base, func_rva);
            out.by_address.entry(addr).or_insert(name);
        }
    }
    out
}

fn parse_imports(
    bytes: &[u8],
    sections: &[PeSectionInfo],
    dir_rva: u32,
    notes: &mut Vec<String>,
) -> (Vec<String>, Vec<(String, String)>) {
    let mut dlls = Vec::new();
    let mut functions = Vec::new();
    if dir_rva == 0 {
        return (dlls, functions);
    }
    let Some(mut desc) = Pe64Image::rva_to_file_offset(sections, dir_rva) else {
        return (dlls, functions);
    };
    let read_name = |rva: u32| {
        Pe64Image::rva_to_file_offset(sections, rva)
            .and_then(|off| read_cstr(bytes, off, MAX_SYMBOL_NAME))
            .unwrap_or_else(|| format!("<unreadable name at RVA 0x{rva:x}>"))
    };

    loop {
        let ilt_rva = read_u32_at(bytes, desc, 0);
        let name_rva = read_u32_at(bytes, desc, 12);
        let iat_rva = read_u32_at(bytes, desc, 16);
        let (Some(ilt_rva), Some(name_rva), Some(iat_rva)) = (ilt_rva, name_rva, iat_rva) else {
            break;
        };
        if ilt_rva == 0 && name_rva == 0 && iat_rva == 0 {
            break;
        }
        let dll = read_name(name_rva);
        let lookup_rva = if ilt_rva != 0 { ilt_rva } else { iat_rva };
        let mut thunk = Pe64Image::rva_to_file_offset(sections, lookup_rva);
        while let Some(entry) = thunk.and_then(|off| read_u64_at(bytes, off, 0)) {
            if entry == 0 {
                break;
            }
            if functions.len() >= MAX_IMPORTED_FUNCTIONS {
                notes.push(format!(
                    "imports cut short after {MAX_IMPORTED_FUNCTIONS} functions"
                ));
                dlls.push(dll);
                return (dlls, functions);
            }
            let function = if entry & (1u64 << 63) != 0 {
                format!("#{}", entry & 0xffff)
            } else {
                // Bits 30..0 hold the RVA of a hint (u16) followed by the name.
                let hint_name_rva = (entry & 0x7fff_ffff) as u32;
                read_name(hint_name_rva.saturating_add(2))
            };
            functions.push((dll.clone(), function));
            thunk = thunk.and_then(|off| off.checked_add(THUNK_SIZE));
        }
        dlls.push(dll);
        let Some(next) = desc.checked_add(IMPORT_DESCRIPTOR_SIZE) else {
            break;
        };
        desc = next;
    }
    (dlls, functions)
}

fn parse_runtime_functions(
    bytes: &[u8],
    sections: &[PeSectionInfo],
    machine: u16,
    (dir_rva, dir_size): (u32, u32),
    notes: &mut Vec<String>,
) -> Vec<RuntimeFunctionEntry> {
    let mut out = Vec::new();
    if dir_rva == 0 || dir_size == 0 {
        return out;
    }
    let entry_size = match machine {
        IMAGE_FILE_MACHINE_AMD64 => X64_RUNTIME_FUNCTION_SIZE,
        IMAGE_FILE_MACHINE_ARM64 => ARM64_RUNTIME_FUNCTION_SIZE,
        _ => {
            notes.push(format!(
                ".pdata not parsed: unsupported machine 0x{machine:04x}"
            ));
            return out;
        }
    };
    let Some(start) = Pe64Image::rva_to_file_offset(sections, dir_rva) else {
        notes.push(".pdata is not backed by file data".into());
        return out;
    };
    let count = usize::try_from(dir_size).unwrap_or(usize::MAX) / entry_size;
    for i in 0..count {
        let off = i.checked_mul(entry_size).and_then(|d| start.checked_add(d));
        let entry = off.and_then(|off| {
            let begin_rva = read_u32_at(bytes, off, 0)?;
            let word = read_u32_at(bytes, off, 4)?;
            if machine == IMAGE_FILE_MACHINE_AMD64 {
                let unwind_data = read_u32_at(bytes, off, 8)?;
                return Some(RuntimeFunctionEntry {
                    begin_rva,
                    end_rva: Some(word),
                    unwind_data,
                });
            }
            let length = arm64_function_length(bytes, sections, word);
            Some(RuntimeFunctionEntry {
                begin_rva,
                end_rva: length.and_then(|len| begin_rva.checked_add(len)),
                unwind_data: word,
            })
        });
        let Some(entry) = entry else {
            notes.push(format!(
                ".pdata truncated: {i} of {count} entries are in the file"
            ));
            break;
        };
        if entry.begin_rva != 0 {
            out.push(entry);
        }
    }
    out
}

/// Length in bytes of an ARM64 function from its `.pdata` unwind word. Packed unwind data
/// (`Flag` 1 or 2 in bits 1..0) holds it in bits 12..2; otherwise the word is the RVA of an
/// `.xdata` record whose first word holds it in bits 17..0. Both count 4-byte instructions.
fn arm64_function_length(bytes: &[u8], sections: &[PeSectionInfo], unwind: u32) -> Option<u32> {
    match unwind & 0b11 {
        1 | 2 => Some(((unwind >> 2) & 0x7ff) * 4),
        0 => {
            let xdata = Pe64Image::rva_to_file_offset(sections, unwind)?;
            let header = read_u32_at(bytes, xdata, 0)?;
            Some((header & 0x3_ffff) * 4)
        }
        _ => None,
    }
}

/// Marks the head byte of each procedure start that lies in a segment's loaded bytes. Returns
/// how many were marked.
fn mark_procedure_starts(
    segments: &mut [Segment],
    image_base: u64,
    entry_point_rva: u32,
    runtime_functions: &[RuntimeFunctionEntry],
) -> usize {
    let starts: BTreeSet<u32> = std::iter::once(entry_point_rva)
        .filter(|&rva| rva != 0)
        .chain(runtime_functions.iter().map(|rf| rf.begin_rva))
        .collect();
    let mut marked = 0;
    for rva in starts {
        let addr = rva_to_va(image_base, rva);
        let Some(seg) = segments.iter_mut().find(|seg| seg.contains(addr)) else {
            continue;
        };
        // Instruction lengths are unknown without a decoder: mark the head byte only.
        if seg.mark_as_procedure(addr, 1) {
            marked += 1;
        }
    }
    marked
}

/// Printable strings from `.rdata`, or `.data` when there is no `.rdata`.
fn scan_notable_strings(bytes: &[u8], sections: &[PeSectionInfo]) -> Vec<String> {
    let section = sections
        .iter()
        .find(|s| s.name == ".rdata")
        .or_else(|| sections.iter().find(|s| s.name == ".data"));
    let Some(sec) = section else {
        return Vec::new();
    };
    let start = usize::try_from(sec.raw_data_ptr).unwrap_or(usize::MAX);
    let len = usize::try_from(sec.raw_data_size).unwrap_or(usize::MAX);
    let end = start
        .saturating_add(len.min(MAX_STRING_SCAN_BYTES))
        .min(bytes.len());
    bytes
        .get(start..end)
        .map(|data| extract_ascii_strings(data, 6, MAX_NOTABLE_STRINGS))
        .unwrap_or_default()
}

fn extract_ascii_strings(bytes: &[u8], min_len: usize, limit: usize) -> Vec<String> {
    bytes
        .split(|&b| b == 0)
        .filter(|part| (min_len..=160).contains(&part.len()))
        .filter(|part| part.iter().all(|&b| is_printable_ascii(b)))
        .take(limit)
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

fn push_if_match(run: &str, keywords: &[String], results: &mut Vec<String>) {
    if (MIN_SCAN_LEN..=MAX_SCAN_LEN).contains(&run.len()) {
        let lower = run.to_lowercase();
        if keywords.iter().any(|k| lower.contains(k.as_str())) {
            results.push(run.to_string());
        }
    }
}

fn rva_to_va(image_base: u64, rva: u32) -> u64 {
    // `parse` rejects an ImageBase that a 32-bit RVA could overflow.
    image_base.saturating_add(u64::from(rva))
}

fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7e).contains(&b)
}

fn read_u16_at(buf: &[u8], base: usize, delta: usize) -> Option<u16> {
    let start = base.checked_add(delta)?;
    let raw: [u8; 2] = buf.get(start..start.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(raw))
}

fn read_u32_at(buf: &[u8], base: usize, delta: usize) -> Option<u32> {
    let start = base.checked_add(delta)?;
    let raw: [u8; 4] = buf.get(start..start.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn read_u64_at(buf: &[u8], base: usize, delta: usize) -> Option<u64> {
    let start = base.checked_add(delta)?;
    let raw: [u8; 8] = buf.get(start..start.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(raw))
}

fn clean_fixed_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// A NUL-terminated printable ASCII string of 1 to `max_len - 1` bytes at `off`.
fn read_cstr(buf: &[u8], off: usize, max_len: usize) -> Option<String> {
    let slice = buf.get(off..off.saturating_add(max_len).min(buf.len()))?;
    let end = slice.iter().position(|&b| b == 0)?;
    let text = &slice[..end];
    if !text.is_empty() && text.iter().all(|&b| is_printable_ascii(b)) {
        Some(String::from_utf8_lossy(text).into_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::ByteType;

    const IMAGE_BASE: u64 = 0x1_4000_0000;
    const COFF_OFF: usize = 0x44;
    const OPT_OFF: usize = 0x58;
    const SEC_TABLE_OFF: usize = OPT_OFF + 240;
    const TEXT_FILE: usize = 0x200;
    /// File offset of RVA 0x2000, the start of `.rdata`.
    const RDATA_FILE: usize = 0x400;

    mod headers {
        use super::*;

        #[test]
        fn parses_headers_and_sections_in_file_order() {
            let pe = parse(&synthetic_pe(IMAGE_FILE_MACHINE_AMD64));
            assert_eq!(pe.machine, IMAGE_FILE_MACHINE_AMD64);
            assert_eq!(pe.arch_name, "x86_64");
            assert_eq!(pe.image_base, IMAGE_BASE);
            assert_eq!(pe.entry_point_rva, 0x1010);
            assert_eq!(pe.va(0x1010), IMAGE_BASE + 0x1010);
            let names: Vec<&str> = pe.sections.iter().map(|s| s.name.as_str()).collect();
            assert_eq!(names, [".rdata", ".text"]);
            assert_eq!(pe.segments[1].start_address, IMAGE_BASE + 0x1000);
            assert_eq!(pe.segments[1].data.len(), 0x200);
            assert!(pe.notes.is_empty(), "{:?}", pe.notes);
            assert!(pe.notable_strings.iter().any(|s| s == "HelloNotable"));
        }

        #[test]
        fn rejects_malformed_headers() {
            assert!(Pe64Image::parse(b"").is_err());

            let mut bad_mz = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            bad_mz[0] = 0;
            assert!(parse_err(&bad_mz).contains("MZ"));

            let mut bad_lfanew = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            put_u32(&mut bad_lfanew, 0x3c, u32::MAX);
            assert!(parse_err(&bad_lfanew).contains("e_lfanew"));

            let mut pe32 = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            put_u16(&mut pe32, OPT_OFF, 0x010B);
            assert!(parse_err(&pe32).contains("PE32+"));

            let mut small_optional = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            put_u16(&mut small_optional, COFF_OFF + 16, 100);
            assert!(parse_err(&small_optional).contains("SizeOfOptionalHeader"));

            let truncated = &synthetic_pe(IMAGE_FILE_MACHINE_AMD64)[..0x100];
            assert!(parse_err(truncated).contains("Truncated optional header"));

            let mut huge_base = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            put_u64(&mut huge_base, OPT_OFF + 24, u64::MAX - 0x1000);
            assert!(parse_err(&huge_base).contains("ImageBase"));
        }

        #[test]
        fn data_directories_beyond_the_optional_header_are_ignored() {
            let mut bytes = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            // Room for the export and import directories only; NumberOfRvaAndSizes still says 16.
            let opt_size = 112 + 2 * 8;
            put_u16(&mut bytes, COFF_OFF + 16, opt_size as u16);
            let table = bytes[SEC_TABLE_OFF..SEC_TABLE_OFF + 80].to_vec();
            put_bytes(&mut bytes, OPT_OFF + opt_size, &table);
            let pe = parse(&bytes);
            assert_eq!(pe.imported_dlls, ["KERNEL32.dll"]);
            assert!(pe.runtime_functions.is_empty());
        }
    }

    mod tables {
        use super::*;

        #[test]
        fn exports_keep_names_aliases_and_forwarders_apart() {
            let pe = parse(&synthetic_pe(IMAGE_FILE_MACHINE_AMD64));
            let expected = BTreeMap::from([
                (IMAGE_BASE + 0x1010, "Alpha".to_string()),
                (IMAGE_BASE + 0x1020, "Beta".to_string()),
            ]);
            assert_eq!(pe.exports, expected);
            assert_eq!(pe.named_export_count, 3);
            let forwarded = [("Fwd".to_string(), "OTHER.Target".to_string())];
            assert_eq!(pe.forwarded_exports, forwarded);
        }

        #[test]
        fn imports_by_name_and_ordinal() {
            let pe = parse(&synthetic_pe(IMAGE_FILE_MACHINE_AMD64));
            assert_eq!(pe.imported_dlls, ["KERNEL32.dll"]);
            let expected = [
                ("KERNEL32.dll".to_string(), "ExitProcess".to_string()),
                ("KERNEL32.dll".to_string(), "#7".to_string()),
            ];
            assert_eq!(pe.imported_functions, expected);
        }

        #[test]
        fn x64_pdata_marks_procedure_heads_in_the_containing_segment() {
            let pe = parse(&synthetic_pe(IMAGE_FILE_MACHINE_AMD64));
            let expected = [
                entry(0x1010, Some(0x1018), 0x2300),
                entry(0x1020, Some(0x1040), 0x2300),
            ];
            assert_eq!(pe.runtime_functions, expected);
            // The entry point is also the first function: two distinct heads.
            assert_eq!(pe.procedures_marked, 2);
            let text = &pe.segments[1];
            let at = |rva: u64| text.get_type_at_address(IMAGE_BASE + rva);
            assert_eq!(at(0x1010), ByteType::Procedure);
            assert_eq!(at(0x1011), ByteType::Undefined);
            assert_eq!(at(0x1020), ByteType::Procedure);
            assert_eq!(text.get_object_length(IMAGE_BASE + 0x1010), 1);
            let rdata = &pe.segments[0];
            assert!(rdata.byte_types.iter().all(|&t| t == ByteType::Undefined));
        }

        #[test]
        fn arm64_pdata_uses_8_byte_entries_and_function_lengths() {
            let mut bytes = synthetic_pe(IMAGE_FILE_MACHINE_ARM64);
            let pdata = RDATA_FILE + 0x200;
            // Packed unwind data (Flag 1) with a FunctionLength of 4 instructions.
            put_u32(&mut bytes, pdata, 0x1010);
            put_u32(&mut bytes, pdata + 4, (4 << 2) | 1);
            // An .xdata record at RVA 0x2300 with a FunctionLength of 8 instructions.
            put_u32(&mut bytes, pdata + 8, 0x1020);
            put_u32(&mut bytes, pdata + 12, 0x2300);
            put_u32(&mut bytes, RDATA_FILE + 0x300, 8);
            put_u32(&mut bytes, data_dir_offset(3) + 4, 16);
            let pe = parse(&bytes);
            assert_eq!(pe.arch_name, "arm64");
            let expected = [
                entry(0x1010, Some(0x1020), 0x11),
                entry(0x1020, Some(0x1040), 0x2300),
            ];
            assert_eq!(pe.runtime_functions, expected);
        }

        #[test]
        fn pdata_of_other_machines_is_skipped_with_a_note() {
            let pe = parse(&synthetic_pe(0x1234));
            assert_eq!(pe.arch_name, "unknown");
            assert!(pe.runtime_functions.is_empty());
            assert!(pe.notes.iter().any(|n| n.contains(".pdata")));
        }
    }

    mod hostile_offsets {
        use super::*;

        #[test]
        fn rva_to_file_offset_does_not_overflow() {
            let sections = [PeSectionInfo {
                name: ".text".into(),
                virtual_address: 0x1000,
                virtual_size: 0x200,
                raw_data_ptr: 0xffff_fff0,
                raw_data_size: 0x200,
                characteristics: 0,
            }];
            assert_eq!(Pe64Image::rva_to_file_offset(&sections, 0x1100), None);
            assert_eq!(
                Pe64Image::rva_to_file_offset(&sections, 0x1008),
                Some(0xffff_fff8)
            );
        }

        #[test]
        fn section_data_outside_the_file_loads_empty() {
            let mut bytes = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            put_u32(&mut bytes, SEC_TABLE_OFF + 40 + 20, 0xffff_fff0);
            let pe = parse(&bytes);
            assert!(pe.segments[1].data.is_empty());
            assert_eq!(pe.procedures_marked, 0);
        }
    }

    mod strings {
        use super::*;

        #[test]
        fn scan_finds_ascii_at_end_of_buffer_and_utf16() {
            let tail = Pe64Image::scan_matching_strings(b"\x00\x01needle-at-end", &["NEEDLE"], 5);
            assert_eq!(tail, ["needle-at-end"]);
            let bytes = synthetic_pe(IMAGE_FILE_MACHINE_AMD64);
            assert_eq!(
                Pe64Image::scan_matching_strings(&bytes, &["widekey"], 5),
                ["WideKey"]
            );
            assert!(Pe64Image::scan_matching_strings(&bytes, &["widekey"], 0).is_empty());
        }
    }

    fn parse(bytes: &[u8]) -> Pe64Image {
        Pe64Image::parse(bytes).unwrap()
    }

    fn parse_err(bytes: &[u8]) -> String {
        Pe64Image::parse(bytes).unwrap_err()
    }

    fn entry(begin_rva: u32, end_rva: Option<u32>, unwind_data: u32) -> RuntimeFunctionEntry {
        RuntimeFunctionEntry {
            begin_rva,
            end_rva,
            unwind_data,
        }
    }

    fn data_dir_offset(index: usize) -> usize {
        OPT_OFF + 112 + index * 8
    }

    /// File offset of an RVA in `.rdata`.
    fn rdata(rva: usize) -> usize {
        RDATA_FILE + rva - 0x2000
    }

    /// A 0x800-byte PE32+ image: `.rdata` (RVA 0x2000, file 0x400) listed before `.text`
    /// (RVA 0x1000, file 0x200), with export, import and x64 `.pdata` tables in `.rdata`.
    fn synthetic_pe(machine: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0x800];
        put_u16(&mut b, 0, MZ_MAGIC);
        put_u32(&mut b, 0x3c, 0x40);
        put_u32(&mut b, 0x40, PE_MAGIC);
        put_u16(&mut b, COFF_OFF, machine);
        put_u16(&mut b, COFF_OFF + 2, 2);
        put_u16(&mut b, COFF_OFF + 16, 240);

        put_u16(&mut b, OPT_OFF, PE32_PLUS_MAGIC);
        put_u32(&mut b, OPT_OFF + 16, 0x1010);
        put_u64(&mut b, OPT_OFF + 24, IMAGE_BASE);
        put_u32(&mut b, OPT_OFF + 108, 16);
        for (index, rva, size) in [(0, 0x2000, 0x100), (1, 0x2100, 40), (3, 0x2200, 24)] {
            put_u32(&mut b, data_dir_offset(index), rva);
            put_u32(&mut b, data_dir_offset(index) + 4, size);
        }

        put_section(&mut b, 0, b".rdata", 0x2000, 0x400, RDATA_FILE);
        put_section(&mut b, 1, b".text", 0x1000, 0x200, TEXT_FILE);
        b[TEXT_FILE..TEXT_FILE + 0x200].fill(0xcc);

        // Export directory at RVA 0x2000 (size 0x100): 3 functions, 4 names.
        put_u32(&mut b, rdata(0x2000) + 20, 3);
        put_u32(&mut b, rdata(0x2000) + 24, 4);
        put_u32(&mut b, rdata(0x2000) + 28, 0x2040);
        put_u32(&mut b, rdata(0x2000) + 32, 0x2050);
        put_u32(&mut b, rdata(0x2000) + 36, 0x2060);
        for (i, func) in [0x1010, 0x1020, 0x2080].into_iter().enumerate() {
            put_u32(&mut b, rdata(0x2040) + i * 4, func);
        }
        for (i, name) in [0x2070, 0x20a0, 0x2078, 0x2090].into_iter().enumerate() {
            put_u32(&mut b, rdata(0x2050) + i * 4, name);
        }
        for (i, ordinal) in [0, 0, 1, 2].into_iter().enumerate() {
            put_u16(&mut b, rdata(0x2060) + i * 2, ordinal);
        }
        put_bytes(&mut b, rdata(0x2070), b"Alpha\0");
        put_bytes(&mut b, rdata(0x2078), b"Beta\0");
        put_bytes(&mut b, rdata(0x2080), b"OTHER.Target\0");
        put_bytes(&mut b, rdata(0x2090), b"Fwd\0");
        put_bytes(&mut b, rdata(0x20a0), b"AlphaAlias\0");

        // Import directory at RVA 0x2100: one descriptor, then the all-zero terminator.
        put_u32(&mut b, rdata(0x2100), 0x2140);
        put_u32(&mut b, rdata(0x2100) + 12, 0x2180);
        put_u32(&mut b, rdata(0x2100) + 16, 0x2160);
        put_u64(&mut b, rdata(0x2140), 0x2190);
        put_u64(&mut b, rdata(0x2148), (1u64 << 63) | 7);
        put_bytes(&mut b, rdata(0x2180), b"KERNEL32.dll\0");
        put_bytes(&mut b, rdata(0x2192), b"ExitProcess\0");

        // x64 .pdata at RVA 0x2200: two RUNTIME_FUNCTION entries.
        let functions = [(0x1010, 0x1018), (0x1020, 0x1040)];
        for (i, (begin, end)) in functions.into_iter().enumerate() {
            let off = rdata(0x2200) + i * 12;
            put_u32(&mut b, off, begin);
            put_u32(&mut b, off + 4, end);
            put_u32(&mut b, off + 8, 0x2300);
        }

        put_bytes(&mut b, rdata(0x2380), b"HelloNotable\0");
        let wide: Vec<u8> = "WideKey"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        put_bytes(&mut b, rdata(0x23c0), &wide);
        b
    }

    fn put_section(b: &mut [u8], index: usize, name: &[u8], rva: u32, size: u32, file: usize) {
        let off = SEC_TABLE_OFF + index * 40;
        put_bytes(b, off, name);
        put_u32(b, off + 8, size);
        put_u32(b, off + 12, rva);
        put_u32(b, off + 16, size);
        put_u32(b, off + 20, file as u32);
    }

    fn put_bytes(b: &mut [u8], off: usize, data: &[u8]) {
        b[off..off + data.len()].copy_from_slice(data);
    }

    fn put_u16(b: &mut [u8], off: usize, v: u16) {
        put_bytes(b, off, &v.to_le_bytes());
    }

    fn put_u32(b: &mut [u8], off: usize, v: u32) {
        put_bytes(b, off, &v.to_le_bytes());
    }

    fn put_u64(b: &mut [u8], off: usize, v: u64) {
        put_bytes(b, off, &v.to_le_bytes());
    }
}
