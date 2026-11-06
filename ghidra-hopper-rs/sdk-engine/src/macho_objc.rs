//! Hopper-Grade Mach-O Universal Binary & Objective-C / Swift Metadata Analyzer in pure Rust.
//!
//! Directly addresses Ghidra's slower Java-based Mach-O / dyld fixup / Objective-C selector
//! analysis by using zero-copy byte slice parsing to extract:
//! - Universal (`0xCAFEBABE`) architecture slices (`x86_64`, `arm64`)
//! - 64-bit Mach-O (`0xFEEDFACF`) segments and sections
//! - Objective-C `__objc_methname`, `__objc_classname`, `__objc_selrefs`, `__objc_stubs`
//! - Swift 5 `__swift5_typeref` and `__swift5_reflstr` reflection strings

use crate::segment::{Section, Segment};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
mod chained;
mod imports;

pub const FAT_MAGIC: u32 = 0xcafebabe;
pub const MH_MAGIC_64: u32 = 0xfeedfacf;
pub const ELF_MAGIC: u32 = 0x7f454c46;
pub const LC_SEGMENT_64: u32 = 0x19;
pub const LC_MAIN: u32 = 0x80000028;

#[derive(Debug, Clone)]
pub struct ArchSlice {
    pub cpu_type: u32,
    pub cpu_subtype: u32,
    pub arch_name: &'static str,
    pub offset: usize,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct ObjCMetadata {
    pub selector_stub_proofs: BTreeMap<u64, SelectorStubProof>,
    pub macho_imports: BTreeMap<u64, MachOImportBinding>,
    pub plt_bindings: BTreeMap<u64, PltBinding>,
    pub class_names: Vec<String>,
    pub method_names: Vec<String>,
    /// Maps executable `__objc_stubs` virtual addresses to verified selector names.
    pub selector_stubs: BTreeMap<u64, String>,
    pub selector_references: BTreeMap<u64, String>,
    /// Ordinary import/ELF symbols must never be lifted as Objective-C calls.
    pub symbols: BTreeMap<u64, String>,
    pub swift_reflection_strings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SelectorStubProof {
    pub context_sha256: String,
    pub instructions: Vec<u32>,
    pub selref: u64,
    pub encoded: u64,
    pub target: u64,
    pub selector: String,
}
impl SelectorStubProof {
    pub fn matches(&self, start: u64, segments: &[Segment]) -> bool {
        self.matches_with_context(start, segments, &selector_context(segments))
    }
    pub(crate) fn matches_with_context(
        &self,
        start: u64,
        segments: &[Segment],
        context: &str,
    ) -> bool {
        let read = |address, size| segments.iter().find_map(|s| s.read_bytes(address, size));
        self.context_sha256 == context
            && self
                .instructions
                .iter()
                .enumerate()
                .all(|(n, w)| read(start + n as u64 * 4, 4) == Some(w.to_le_bytes().as_slice()))
            && read(self.selref, 8) == Some(self.encoded.to_le_bytes().as_slice())
            && read(self.target, self.selector.len() + 1).is_some_and(|b| {
                b.last() == Some(&0) && &b[..b.len() - 1] == self.selector.as_bytes()
            })
    }
}

pub(crate) fn selector_context(segments: &[Segment]) -> String {
    let mut hash = Sha256::new();
    for s in segments {
        hash.update(s.start_address.to_le_bytes());
        hash.update(s.file_offset.to_le_bytes());
        hash.update([s.permissions.unwrap_or(255)]);
        hash.update((s.data.len() as u64).to_le_bytes());
        hash.update(&s.data);
    }
    format!("{:x}", hash.finalize())
}

#[derive(Debug, Clone)]
pub struct PltBinding {
    pub instructions: [u32; 4],
    pub stub_address: u64,
    pub branch_address: u64,
    pub got_address: u64,
    pub symbol: String,
}

#[derive(Debug, Clone)]
pub struct MachOImportBinding {
    pub library_ordinal: u16,
    pub context_sha256: String,
    pub stub_address: u64,
    pub pointer_address: u64,
    pub symbol: String,
    pub library: String,
    pub instructions: Vec<u32>,
    pub selector_address: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct MachOImage {
    pub is_universal: bool,
    pub slices: Vec<ArchSlice>,
    pub selected_arch: &'static str,
    pub entry_point: Option<u64>,
    pub segments: Vec<Segment>,
    pub objc: ObjCMetadata,
}

fn read_u32_be(buf: &[u8], off: usize) -> Option<u32> {
    let s = buf.get(off..off.checked_add(4)?)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u16_le(buf: &[u8], off: usize) -> Option<u16> {
    let s = buf.get(off..off.checked_add(2)?)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32_le(buf: &[u8], off: usize) -> Option<u32> {
    let s = buf.get(off..off.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u64_le(buf: &[u8], off: usize) -> Option<u64> {
    let s = buf.get(off..off.checked_add(8)?)?;
    Some(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

fn clean_fixed_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn cpu_type_name(cpu_type: u32) -> &'static str {
    match cpu_type {
        0x01000007 => "x86_64",
        0x0100000c => "arm64",
        7 => "i386",
        12 => "arm",
        _ => "unknown",
    }
}

fn checked_range(
    offset: usize,
    size: usize,
    total: usize,
) -> Result<std::ops::Range<usize>, String> {
    let end = offset.checked_add(size).ok_or("Binary range overflow")?;
    if end > total {
        return Err("Binary range out of bounds".into());
    }
    Ok(offset..end)
}

fn validate_table(
    offset: usize,
    count: usize,
    stride: usize,
    minimum: usize,
    total: usize,
) -> Result<(), String> {
    if count == 0 {
        return Ok(());
    }
    if stride < minimum {
        return Err("Invalid binary table entry size".into());
    }
    let size = count
        .checked_mul(stride)
        .ok_or("Binary table size overflow")?;
    checked_range(offset, size, total)?;
    Ok(())
}

fn record_function_starts(data: &[u8], base: u64, segments: &mut [Segment]) -> Result<(), String> {
    let mut cursor = 0;
    let mut address = base;
    while cursor < data.len() {
        let mut delta = 0u64;
        let mut shift = 0;
        loop {
            let byte = *data.get(cursor).ok_or("Truncated function-start ULEB128")?;
            cursor += 1;
            if shift > 63 || (shift == 63 && byte & 0x7f > 1) {
                return Err("Function-start ULEB128 overflow".into());
            }
            delta |= ((byte & 127) as u64) << shift;
            if byte & 128 == 0 {
                break;
            }
            shift += 7;
        }
        if delta == 0 {
            if data[cursor..].iter().any(|&b| b != 0) {
                return Err("Invalid function-start padding".into());
            }
            return Ok(());
        }
        address = address
            .checked_add(delta)
            .ok_or("Function-start address overflow")?;
        if !address.is_multiple_of(4) {
            return Err("Unaligned ARM64 function start".into());
        }
        let segment = segments
            .iter_mut()
            .find(|s| {
                s.permissions.is_some_and(|p| p & 4 != 0) && s.read_bytes(address, 4).is_some()
            })
            .ok_or("Function start outside executable mapping")?;
        segment.function_starts.insert(address);
    }
    if data.is_empty() {
        Ok(())
    } else {
        Err("Unterminated function-start table".into())
    }
}

impl MachOImage {
    /// Parses either a Universal Mach-O (`0xCAFEBABE`) or a thin 64-bit Mach-O (`0xFEEDFACF`),
    /// preferring the `arm64` slice on Apple Silicon just like Hopper Disassembler.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 32 {
            return Err("Binary too small for Mach-O header".into());
        }
        let magic_be = read_u32_be(bytes, 0).unwrap_or(0);
        let magic_le = read_u32_le(bytes, 0).unwrap_or(0);

        let mut slices = Vec::new();
        if magic_be == ELF_MAGIC {
            return Self::parse_elf64(bytes);
        }
        let (is_universal, slice_bytes, selected_arch) = if magic_be == FAT_MAGIC {
            let nfat_arch = read_u32_be(bytes, 4).ok_or("Truncated fat_header")? as usize;
            if nfat_arch == 0 {
                return Err("Empty universal architecture table".into());
            }
            validate_table(8, nfat_arch, 20, 20, bytes.len())?;
            let mut chosen_off = 0usize;
            let mut chosen_size = 0usize;
            let mut chosen_arch = "unknown";
            for i in 0..nfat_arch {
                let base = 8 + i * 20;
                let cpu_type = read_u32_be(bytes, base).ok_or("Truncated fat_arch")?;
                let cpu_subtype = read_u32_be(bytes, base + 4).ok_or("Truncated fat_arch")?;
                let offset = read_u32_be(bytes, base + 8).ok_or("Truncated fat_arch")? as usize;
                let size = read_u32_be(bytes, base + 12).ok_or("Truncated fat_arch")? as usize;
                checked_range(offset, size, bytes.len())?;
                let arch_name = cpu_type_name(cpu_type);
                slices.push(ArchSlice {
                    cpu_type,
                    cpu_subtype,
                    arch_name,
                    offset,
                    size,
                });
                if chosen_size == 0 || arch_name == "arm64" {
                    chosen_off = offset;
                    chosen_size = size;
                    chosen_arch = arch_name;
                }
            }
            let sub = bytes
                .get(chosen_off..chosen_off.saturating_add(chosen_size))
                .ok_or("Universal slice out of bounds")?;
            (true, sub, chosen_arch)
        } else if magic_le == MH_MAGIC_64 {
            let cpu_type = read_u32_le(bytes, 4).unwrap_or(0);
            let cpu_subtype = read_u32_le(bytes, 8).unwrap_or(0);
            let arch_name = cpu_type_name(cpu_type);
            slices.push(ArchSlice {
                cpu_type,
                cpu_subtype,
                arch_name,
                offset: 0,
                size: bytes.len(),
            });
            (false, bytes, arch_name)
        } else {
            return Err(format!("Unsupported binary magic: 0x{:08x}", magic_be));
        };

        Self::parse_thin_macho64(slice_bytes, is_universal, slices, selected_arch)
    }

    fn parse_thin_macho64(
        slice: &[u8],
        is_universal: bool,
        slices: Vec<ArchSlice>,
        selected_arch: &'static str,
    ) -> Result<Self, String> {
        let magic = read_u32_le(slice, 0).ok_or("Missing Mach-O 64 magic")?;
        if magic != MH_MAGIC_64 {
            return Err(format!(
                "Expected MH_MAGIC_64 (0xfeedfacf), got 0x{:08x}",
                magic
            ));
        }
        let ncmds = read_u32_le(slice, 16).ok_or("Missing ncmds")? as usize;
        let sizeofcmds = read_u32_le(slice, 20).ok_or("Missing sizeofcmds")? as usize;
        let command_end = checked_range(32, sizeofcmds, slice.len())?.end;
        if ncmds > sizeofcmds / 8 {
            return Err("Invalid Mach-O load command count".into());
        }
        let mut cmd_offset = 32usize;

        let mut segments = Vec::new();
        let mut entry_point = None;
        let mut text_vmaddr = 0u64;
        let mut function_start_data = None;
        let mut chained_data = None;

        let mut objc = ObjCMetadata {
            selector_stub_proofs: BTreeMap::new(),
            macho_imports: BTreeMap::new(),
            plt_bindings: BTreeMap::new(),
            class_names: Vec::new(),
            method_names: Vec::new(),
            selector_stubs: BTreeMap::new(),
            selector_references: BTreeMap::new(),
            symbols: BTreeMap::new(),
            swift_reflection_strings: Vec::new(),
        };

        // Track section ranges so we can recover ObjC/Swift tables after segments are loaded.
        let mut sec_ranges: Vec<(String, u64, usize, usize)> = Vec::new();

        for _ in 0..ncmds {
            checked_range(cmd_offset, 8, command_end)?;
            let cmd = read_u32_le(slice, cmd_offset).ok_or("Truncated load command")?;
            let cmdsize =
                read_u32_le(slice, cmd_offset + 4).ok_or("Truncated load command")? as usize;
            if cmdsize < 8 || !cmdsize.is_multiple_of(8) {
                return Err("Invalid Mach-O command size".into());
            }
            checked_range(cmd_offset, cmdsize, command_end)?;
            if (cmd == LC_SEGMENT_64 && cmdsize < 72) || (cmd == LC_MAIN && cmdsize < 24) {
                return Err("Truncated Mach-O load command body".into());
            }

            if cmd == LC_SEGMENT_64 && cmdsize >= 72 {
                let segname = clean_fixed_str(&slice[cmd_offset + 8..cmd_offset + 24]);
                let vmaddr = read_u64_le(slice, cmd_offset + 24).unwrap_or(0);
                let _vmsize = read_u64_le(slice, cmd_offset + 32).unwrap_or(0);
                let fileoff = read_u64_le(slice, cmd_offset + 40).unwrap_or(0) as usize;
                let filesize = read_u64_le(slice, cmd_offset + 48).unwrap_or(0) as usize;
                let nsects = read_u32_le(slice, cmd_offset + 64).unwrap_or(0) as usize;
                validate_table(cmd_offset + 72, nsects, 80, 80, cmd_offset + cmdsize)?;
                checked_range(fileoff, filesize, slice.len())?;
                vmaddr
                    .checked_add(filesize as u64)
                    .ok_or("Segment address overflow")?;

                if segname == "__TEXT" {
                    text_vmaddr = vmaddr;
                }

                let seg_bytes = if fileoff < slice.len() {
                    let end = fileoff + filesize;
                    slice[fileoff..end].to_vec()
                } else {
                    Vec::new()
                };

                let mut seg = Segment::new(segname.clone(), vmaddr, fileoff as u64, seg_bytes);
                seg.permissions =
                    Some((read_u32_le(slice, cmd_offset + 60).unwrap_or(0) & 7) as u8);

                let mut sect_cursor = cmd_offset + 72;
                for _ in 0..nsects {
                    if sect_cursor + 80 > cmd_offset + cmdsize as usize {
                        break;
                    }
                    let sectname = clean_fixed_str(&slice[sect_cursor..sect_cursor + 16]);
                    let addr = read_u64_le(slice, sect_cursor + 32).unwrap_or(0);
                    let size = read_u64_le(slice, sect_cursor + 40).unwrap_or(0);
                    let offset = read_u32_le(slice, sect_cursor + 48).unwrap_or(0) as usize;
                    let flags = read_u32_le(slice, sect_cursor + 64).unwrap_or(0);
                    addr.checked_add(size).ok_or("Section address overflow")?;
                    // Zerofill sections have no corresponding bytes in the file.
                    let file_backed = !matches!(flags & 0xff, 1 | 0xc | 0x12);
                    if file_backed {
                        checked_range(offset, size as usize, slice.len())?;
                    }

                    seg.sections.push(Section {
                        name: sectname.clone(),
                        start_address: addr,
                        length: size,
                        flags,
                    });
                    if file_backed {
                        sec_ranges.push((sectname, addr, offset, size as usize));
                    }
                    sect_cursor += 80;
                }
                segments.push(seg);
            } else if cmd == LC_MAIN && cmdsize >= 24 {
                let entryoff = read_u64_le(slice, cmd_offset + 8).unwrap_or(0);
                entry_point = Some(entryoff);
            } else if cmd == 0x80000034 {
                if cmdsize < 16 || chained_data.is_some() {
                    return Err("Invalid LC_DYLD_CHAINED_FIXUPS".into());
                }
                let off = read_u32_le(slice, cmd_offset + 8).unwrap() as usize;
                let size = read_u32_le(slice, cmd_offset + 12).unwrap() as usize;
                chained_data = Some(checked_range(off, size, slice.len())?);
            } else if cmd == 0x26 {
                if cmdsize < 16 || function_start_data.is_some() {
                    return Err("Invalid LC_FUNCTION_STARTS command".into());
                }
                let off = read_u32_le(slice, cmd_offset + 8).unwrap() as usize;
                let size = read_u32_le(slice, cmd_offset + 12).unwrap() as usize;
                function_start_data = Some(checked_range(off, size, slice.len())?);
            }

            cmd_offset += cmdsize as usize;
        }
        if cmd_offset != command_end {
            return Err("Mach-O sizeofcmds does not match commands".into());
        }

        if let Some(ep_off) = entry_point {
            entry_point = Some(
                text_vmaddr
                    .checked_add(ep_off)
                    .ok_or("Entry address overflow")?,
            );
        }

        // Pass 1: Extract Objective-C & Swift string tables first (`__objc_methname` often appears after `__objc_stubs`)
        for (sectname, _addr, offset, size) in &sec_ranges {
            let Some(bytes_sec) = slice.get(*offset..offset.saturating_add((*size).min(262_144)))
            else {
                continue;
            };
            match sectname.as_str() {
                "__objc_classname" => {
                    objc.class_names = extract_null_terminated_strings(bytes_sec, 120);
                }
                "__objc_methname" => {
                    objc.method_names = extract_null_terminated_strings(bytes_sec, 200);
                }
                "__swift5_reflstr" | "__swift5_typeref" => {
                    let mut s = extract_null_terminated_strings(bytes_sec, 80);
                    objc.swift_reflection_strings.append(&mut s);
                }
                _ => {}
            }
        }

        let rebases = chained_data
            .as_ref()
            .map(|range| chained::decode(&slice[range.clone()], &segments, text_vmaddr))
            .transpose()?;
        let mut selector_targets = BTreeMap::new();
        // Resolve selectors through their actual VM pointers. String table order is unrelated
        // to selref/stub order; unresolved dyld chained pointers must not become invented names.
        let resolve_selector = |pointer: u64| -> Option<String> {
            for (name, addr, offset, size) in &sec_ranges {
                if name != "__objc_methname" {
                    continue;
                }
                let Some(delta) = pointer
                    .checked_sub(*addr)
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    continue;
                };
                if delta >= *size {
                    continue;
                }
                if !segments.iter().any(|s| {
                    s.permissions.is_some_and(|p| p & 1 != 0)
                        && s.file_offset_for_address(pointer) == Some((*offset + delta) as u64)
                }) {
                    continue;
                }
                let tail = slice.get(offset.checked_add(delta)?..offset.checked_add(*size)?)?;
                let end = tail.iter().position(|&b| b == 0)?;
                if end == 0 {
                    return None;
                }
                if !segments
                    .iter()
                    .any(|s| s.read_bytes(pointer, end + 1) == Some(&tail[..end + 1]))
                {
                    continue;
                }
                return std::str::from_utf8(&tail[..end]).ok().map(str::to_owned);
            }
            None
        };
        for (name, addr, offset, size) in &sec_ranges {
            if name != "__objc_selrefs" {
                continue;
            }
            let bytes_sec = &slice[*offset..offset + size];
            for (idx, ptr) in bytes_sec.chunks_exact(8).enumerate() {
                let pointer = u64::from_le_bytes(ptr.try_into().unwrap());
                let slot = addr + (idx * 8) as u64;
                if !segments.iter().any(|s| {
                    s.file_offset_for_address(slot) == Some((*offset + idx * 8) as u64)
                        && s.permissions.is_some_and(|p| p & 1 != 0)
                        && s.read_bytes(slot, 8) == Some(ptr)
                }) {
                    continue;
                }
                let pointer = if let Some(rebases) = &rebases {
                    let Some(pointer) = rebases.rebases.get(&slot) else {
                        continue;
                    };
                    *pointer
                } else {
                    pointer
                };
                if let Some(sel) = resolve_selector(pointer) {
                    selector_targets.insert(slot, pointer);
                    objc.selector_references
                        .insert(addr + (idx * 8) as u64, sel);
                }
            }
        }

        if selected_arch == "arm64" {
            if let Some(range) = function_start_data {
                record_function_starts(&slice[range], text_vmaddr, &mut segments)?;
            }
            let context = selector_context(&segments);
            objc.macho_imports = imports::resolve(slice, &segments, &context)?;
            if let Some(chains) = &rebases {
                objc.macho_imports.retain(|_, b| {
                    chains
                        .binds
                        .get(&b.pointer_address)
                        .is_some_and(|(name, ordinal)| {
                            *name == b.symbol && *ordinal == b.library_ordinal
                        })
                });
            }
            for binding in objc.macho_imports.values() {
                if let Some(selref) = binding.selector_address {
                    if let (Some(selector), Some(&target)) = (
                        objc.selector_references.get(&selref),
                        selector_targets.get(&selref),
                    ) {
                        if let Some(encoded) = segments.iter().find_map(|s| s.read_u64_le(selref)) {
                            let proof = SelectorStubProof {
                                context_sha256: context.clone(),
                                instructions: binding.instructions.clone(),
                                selref,
                                encoded,
                                target,
                                selector: selector.clone(),
                            };
                            if proof.matches_with_context(binding.stub_address, &segments, &context)
                            {
                                objc.selector_stubs
                                    .insert(binding.stub_address, selector.clone());
                                objc.selector_stub_proofs
                                    .insert(binding.stub_address, proof);
                            }
                        }
                    }
                }
                objc.symbols
                    .insert(binding.stub_address, format!("import${}", binding.symbol));
            }
        }
        Ok(Self {
            is_universal,
            slices,
            selected_arch,
            entry_point,
            segments,
            objc,
        })
    }

    /// Zero-copy ELF64 (`.so` / executable) loader for Android NDK / Linux ARM64 & x86_64 binaries.
    /// Extracts `PT_LOAD` segments, section headers, `.dynsym`/`.dynstr` JNI & C++ exports,
    /// `.rela.plt` -> `.plt` AArch64 call stubs, and `.note.gnu.build-id`.
    fn parse_elf64(slice: &[u8]) -> Result<Self, String> {
        if slice.len() < 64 {
            return Err("Binary too small for ELF64 header".into());
        }
        let ei_class = slice[4];
        if ei_class != 2 {
            return Err(format!(
                "Unsupported ELF class {} (expected ELF64=2)",
                ei_class
            ));
        }
        if slice[5] != 1 {
            return Err("Only little-endian ELF64 is supported".into());
        }
        let e_machine = read_u16_le(slice, 18).unwrap_or(0);
        let selected_arch = match e_machine {
            183 => "arm64",
            62 => "x86_64",
            40 => "arm",
            3 => "i386",
            _ => "unknown",
        };
        let e_entry = read_u64_le(slice, 24).unwrap_or(0);
        let e_phoff = read_u64_le(slice, 32).unwrap_or(0) as usize;
        let e_shoff = read_u64_le(slice, 40).unwrap_or(0) as usize;
        let e_phentsize = read_u16_le(slice, 54).unwrap_or(0) as usize;
        let e_phnum = read_u16_le(slice, 56).unwrap_or(0) as usize;
        let e_shentsize = read_u16_le(slice, 58).unwrap_or(0) as usize;
        let e_shnum = read_u16_le(slice, 60).unwrap_or(0) as usize;
        let e_shstrndx = read_u16_le(slice, 62).unwrap_or(0) as usize;
        validate_table(e_phoff, e_phnum, e_phentsize, 56, slice.len())?;
        validate_table(e_shoff, e_shnum, e_shentsize, 64, slice.len())?;
        if e_shstrndx != 0 && e_shstrndx >= e_shnum {
            return Err("Invalid ELF section string table index".into());
        }

        let slices = vec![ArchSlice {
            cpu_type: e_machine as u32,
            cpu_subtype: 0,
            arch_name: selected_arch,
            offset: 0,
            size: slice.len(),
        }];

        let mut segments = Vec::new();
        if e_phentsize >= 56 {
            for i in 0..e_phnum {
                let off = e_phoff + i * e_phentsize;
                if off + 56 > slice.len() {
                    break;
                }
                let p_type = read_u32_le(slice, off).unwrap_or(0);
                if p_type != 1 {
                    continue; // PT_LOAD == 1
                }
                let p_flags = read_u32_le(slice, off + 4).unwrap_or(0);
                let p_offset = read_u64_le(slice, off + 8).unwrap_or(0) as usize;
                let p_vaddr = read_u64_le(slice, off + 16).unwrap_or(0);
                let p_filesz = read_u64_le(slice, off + 32).unwrap_or(0) as usize;
                checked_range(p_offset, p_filesz, slice.len())?;
                p_vaddr
                    .checked_add(p_filesz as u64)
                    .ok_or("ELF segment address overflow")?;

                let segname = if segments.is_empty() {
                    "__TEXT".to_string()
                } else {
                    format!("PT_LOAD_{}", segments.len())
                };
                let seg_bytes = if p_offset < slice.len() {
                    let end = p_offset + p_filesz;
                    slice[p_offset..end].to_vec()
                } else {
                    Vec::new()
                };
                let mut seg = Segment::new(segname, p_vaddr, p_offset as u64, seg_bytes);
                seg.permissions =
                    Some((((p_flags & 4) >> 2) | (p_flags & 2) | ((p_flags & 1) << 2)) as u8);
                segments.push(seg);
            }
        }

        let mut objc = ObjCMetadata {
            macho_imports: BTreeMap::new(),
            selector_stub_proofs: BTreeMap::new(),
            plt_bindings: BTreeMap::new(),
            class_names: Vec::new(),
            method_names: Vec::new(),
            selector_stubs: BTreeMap::new(),
            selector_references: BTreeMap::new(),
            symbols: BTreeMap::new(),
            swift_reflection_strings: Vec::new(),
        };

        let shstrtab: &[u8] = if e_shentsize >= 64 && e_shstrndx < e_shnum {
            let sh_off = e_shoff + e_shstrndx * e_shentsize;
            let off = read_u64_le(slice, sh_off + 24).unwrap_or(0) as usize;
            let sz = read_u64_le(slice, sh_off + 32).unwrap_or(0) as usize;
            slice.get(off..off.saturating_add(sz)).unwrap_or(&[])
        } else {
            &[]
        };

        let mut dynsym_range = (0usize, 0usize);
        let mut dynstr_range = (0usize, 0usize);
        let mut rela_plt_range = (0usize, 0usize);
        let mut plt_addr_size = (0u64, 0usize);
        let mut plt_file_offset = 0usize;
        let mut dynsym_index = None;
        let mut dynstr_index = None;
        let mut dynsym_link = 0;
        let mut rela_link = 0;
        let mut valid_rela = false;
        let mut valid_sym = false;
        let mut first_jni_entry: Option<u64> = None;

        if e_shentsize >= 64 {
            for i in 0..e_shnum {
                let sh_off = e_shoff + i * e_shentsize;
                if sh_off + 64 > slice.len() {
                    break;
                }
                let sh_name_idx = read_u32_le(slice, sh_off).unwrap_or(0) as usize;
                let sh_flags = read_u64_le(slice, sh_off + 8).unwrap_or(0) as u32;
                let sh_addr = read_u64_le(slice, sh_off + 16).unwrap_or(0);
                let sh_offset = read_u64_le(slice, sh_off + 24).unwrap_or(0) as usize;
                let sh_size = read_u64_le(slice, sh_off + 32).unwrap_or(0) as usize;
                let sh_type = read_u32_le(slice, sh_off + 4).unwrap_or(0);
                if sh_type != 8 {
                    checked_range(sh_offset, sh_size, slice.len())?;
                }

                let sectname = if sh_name_idx < shstrtab.len() {
                    clean_fixed_str(&shstrtab[sh_name_idx..])
                } else {
                    format!("sec_{}", i)
                };

                if !sectname.is_empty() {
                    if let Some(seg) = segments.first_mut() {
                        seg.sections.push(Section {
                            name: sectname.clone(),
                            start_address: sh_addr,
                            length: sh_size as u64,
                            flags: sh_flags,
                        });
                    }
                }

                match sectname.as_str() {
                    ".dynsym" => {
                        dynsym_range = (sh_offset, sh_size);
                        dynsym_index = Some(i);
                        dynsym_link = read_u32_le(slice, sh_off + 40).unwrap_or(0) as usize;
                        valid_sym = sh_type == 11
                            && read_u64_le(slice, sh_off + 56) == Some(24)
                            && sh_size.is_multiple_of(24);
                    }
                    ".dynstr" => {
                        dynstr_range = (sh_offset, sh_size);
                        dynstr_index = Some(i);
                    }
                    ".rela.plt" => {
                        rela_plt_range = (sh_offset, sh_size);
                        rela_link = read_u32_le(slice, sh_off + 40).unwrap_or(0) as usize;
                        valid_rela = sh_type == 4
                            && read_u64_le(slice, sh_off + 56) == Some(24)
                            && sh_size.is_multiple_of(24);
                    }
                    ".plt" => {
                        plt_addr_size = (sh_addr, sh_size);
                        plt_file_offset = sh_offset;
                    }
                    ".note.gnu.build-id" => {
                        if let Some(note) = slice.get(sh_offset..sh_offset.saturating_add(sh_size))
                        {
                            if note.len() >= 16 {
                                let namesz = read_u32_le(note, 0).unwrap_or(0) as usize;
                                let descsz = read_u32_le(note, 4).unwrap_or(0) as usize;
                                let aligned_name = (namesz + 3) & !3;
                                let desc_start = 12 + aligned_name;
                                if let Some(desc) =
                                    note.get(desc_start..desc_start.saturating_add(descsz))
                                {
                                    let hex: String =
                                        desc.iter().map(|b| format!("{:02x}", b)).collect();
                                    objc.swift_reflection_strings
                                        .push(format!("gnu.build-id:{}", hex));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let dynstr = slice
            .get(dynstr_range.0..dynstr_range.0.saturating_add(dynstr_range.1))
            .unwrap_or(&[]);
        let dynsym = slice
            .get(dynsym_range.0..dynsym_range.0.saturating_add(dynsym_range.1))
            .unwrap_or(&[]);

        let sym_count = dynsym.len() / 24;
        let mut sym_names = Vec::with_capacity(sym_count);
        for i in 0..sym_count {
            let base = i * 24;
            let st_name = read_u32_le(dynsym, base).unwrap_or(0) as usize;
            let st_value = read_u64_le(dynsym, base + 8).unwrap_or(0);
            if dynsym[base + 4] & 15 == 2
                && read_u16_le(dynsym, base + 6).unwrap_or(0) != 0
                && st_value != 0
            {
                if let Some(segment) = segments.iter_mut().find(|s| s.contains(st_value)) {
                    segment.function_starts.insert(st_value);
                }
            }
            let name = if st_name < dynstr.len() {
                clean_fixed_str(&dynstr[st_name..])
            } else {
                String::new()
            };
            if st_value != 0 && !name.is_empty() {
                objc.symbols.insert(st_value, name.clone());
                if name == "JNI_OnLoad" && first_jni_entry.is_none() {
                    first_jni_entry = Some(st_value);
                }
                if name.starts_with("Java_") || name.starts_with("JNI_") {
                    objc.method_names.push(name.clone());
                    if let Some(rest) = name.strip_prefix("Java_") {
                        if let Some((cls_part, _)) = rest.rsplit_once('_') {
                            let cls = cls_part.replace('_', ".");
                            if !objc.class_names.contains(&cls) {
                                objc.class_names.push(cls);
                            }
                        }
                    }
                }
            }
            let valid_name = dynstr
                .get(st_name..)
                .and_then(|tail| {
                    tail.iter()
                        .position(|b| *b == 0)
                        .and_then(|end| std::str::from_utf8(&tail[..end]).ok())
                })
                .unwrap_or("");
            sym_names.push(valid_name.to_string());
        }

        // Match actual ADRP/LDR/ADD/BR code to a JUMP_SLOT's GOT address.
        // Never assume relocation order, PLT stride, or that a lazy GOT value is its final target.
        if selected_arch == "arm64"
            && valid_sym
            && valid_rela
            && dynsym_index == Some(rela_link)
            && dynstr_index == Some(dynsym_link)
            && plt_addr_size.0 != 0
            && rela_plt_range.1 >= 24
        {
            if let Some(rela_bytes) =
                slice.get(rela_plt_range.0..rela_plt_range.0.saturating_add(rela_plt_range.1))
            {
                let rela_count = rela_bytes.len() / 24;
                let mut slots = BTreeMap::new();
                for i in 0..rela_count {
                    let base = i * 24;
                    let r_info = read_u64_le(rela_bytes, base + 8).unwrap_or(0);
                    let sym_idx = (r_info >> 32) as usize;
                    if r_info as u32 != 1026 || read_u64_le(rela_bytes, base + 16) != Some(0) {
                        continue;
                    }
                    let got = read_u64_le(rela_bytes, base).unwrap_or(0);
                    if !segments.iter().any(|seg| seg.read_bytes(got, 8).is_some()) {
                        continue;
                    }
                    if let Some(sym_name) = sym_names.get(sym_idx) {
                        if !sym_name.is_empty() {
                            slots
                                .entry(got)
                                .or_insert_with(Vec::new)
                                .push(sym_name.clone());
                        }
                    }
                }
                let plt = &slice[checked_range(plt_file_offset, plt_addr_size.1, slice.len())?];
                for offset in (0..plt.len().saturating_sub(15)).step_by(4) {
                    let a = read_u32_le(plt, offset).unwrap();
                    let l = read_u32_le(plt, offset + 4).unwrap();
                    let add = read_u32_le(plt, offset + 8).unwrap();
                    let br = read_u32_le(plt, offset + 12).unwrap();
                    if a & 0x9f00001f != 0x90000010
                        || l & 0xffc003ff != 0xf9400211
                        || add & 0xffc003ff != 0x91000210
                        || br != 0xd61f0220
                    {
                        continue;
                    }
                    let Some(pc) = plt_addr_size.0.checked_add(offset as u64) else {
                        continue;
                    };
                    if !segments
                        .iter()
                        .any(|seg| seg.read_bytes(pc, 16) == Some(&plt[offset..offset + 16]))
                    {
                        continue;
                    }
                    let imm = ((((a >> 5) & 0x7ffff) << 2) | ((a >> 29) & 3)) as i64;
                    let page = (pc & !0xfff).wrapping_add((((imm << 43) >> 43) << 12) as u64);
                    let displacement = (((l >> 10) & 4095) * 8) as u64;
                    if ((add >> 10) & 4095) as u64 != displacement {
                        continue;
                    }
                    let got = page.wrapping_add(displacement);
                    let Some(names) = slots.get(&got).filter(|names| names.len() == 1) else {
                        continue;
                    };
                    let Some(branch_address) = pc.checked_add(12) else {
                        continue;
                    };
                    let symbol = names[0].clone();
                    objc.symbols.insert(pc, format!("plt${symbol}"));
                    objc.plt_bindings.insert(
                        branch_address,
                        PltBinding {
                            instructions: [a, l, add, br],
                            stub_address: pc,
                            branch_address,
                            got_address: got,
                            symbol,
                        },
                    );
                }
            }
        }

        let entry_point = if e_entry != 0 {
            Some(e_entry)
        } else {
            first_jni_entry
        };

        Ok(Self {
            is_universal: false,
            slices,
            selected_arch,
            entry_point,
            segments,
            objc,
        })
    }
}

fn extract_null_terminated_strings(bytes: &[u8], limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    for part in bytes.split(|&b| b == 0) {
        if part.len() >= 2 && part.iter().all(|&b| (0x20..=0x7e).contains(&b)) {
            out.push(String::from_utf8_lossy(part).to_string());
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}
