//! Canonical ARM64 Mach-O import stubs. File pointer values are deliberately
//! ignored: dyld rebasing, binding, and chained fixups determine runtime values.
use super::{
    checked_range, clean_fixed_str, read_u16_le, read_u32_le, read_u64_le, validate_table,
    MachOImportBinding,
};
use crate::segment::Segment;
use std::collections::BTreeMap;

struct IndirectSection {
    name: String,
    address: u64,
    offset: usize,
    size: usize,
    kind: u32,
    first: usize,
    stride: usize,
}

fn cstring(bytes: &[u8], offset: usize) -> Option<String> {
    let rest = bytes.get(offset..)?;
    let end = rest.iter().position(|&c| c == 0)?;
    let text = std::str::from_utf8(&rest[..end]).ok()?;
    (!text.is_empty()).then(|| text.to_owned())
}

pub(super) fn resolve(
    slice: &[u8],
    segments: &[Segment],
    context: &str,
) -> Result<BTreeMap<u64, MachOImportBinding>, String> {
    let mut bindings = BTreeMap::new();
    // Only the two-level namespace's explicit dependency ordinals are resolved.
    if read_u32_le(slice, 4) != Some(0x0100000c) || read_u32_le(slice, 24).unwrap_or(0) & 0x80 == 0
    {
        return Ok(bindings);
    }
    let mut symbols = None;
    let mut indirect = None;
    let mut libraries = Vec::new();
    let mut sections = Vec::new();
    let mut command = 32usize;
    let end = checked_range(
        32,
        read_u32_le(slice, 20).unwrap_or(0) as usize,
        slice.len(),
    )?
    .end;
    for _ in 0..read_u32_le(slice, 16).unwrap_or(0) {
        checked_range(command, 8, end)?;
        let cmd = read_u32_le(slice, command).unwrap();
        let size = read_u32_le(slice, command + 4).unwrap() as usize;
        if size < 8 {
            return Err("Invalid Mach-O import command size".into());
        }
        checked_range(command, size, end)?;
        match cmd {
            2 => {
                // LC_SYMTAB
                if size < 24 || symbols.is_some() {
                    return Err("Invalid or duplicate LC_SYMTAB".into());
                }
                let offset = read_u32_le(slice, command + 8).unwrap() as usize;
                let count = read_u32_le(slice, command + 12).unwrap() as usize;
                let strings = read_u32_le(slice, command + 16).unwrap() as usize;
                let string_size = read_u32_le(slice, command + 20).unwrap() as usize;
                validate_table(offset, count, 16, 16, slice.len())?;
                checked_range(strings, string_size, slice.len())?;
                symbols = Some((offset, count, strings, string_size));
            }
            11 => {
                // LC_DYSYMTAB: indirectsymoff, nindirectsyms
                if size < 80 || indirect.is_some() {
                    return Err("Invalid or duplicate LC_DYSYMTAB".into());
                }
                let offset = read_u32_le(slice, command + 56).unwrap() as usize;
                let count = read_u32_le(slice, command + 60).unwrap() as usize;
                validate_table(offset, count, 4, 4, slice.len())?;
                indirect = Some((offset, count));
            }
            0xc | 0x80000018 | 0x8000001f | 0x80000023 => {
                // LOAD, LOAD_WEAK, REEXPORT, LOAD_UPWARD (ID_DYLIB is not a dependency).
                if size < 24 {
                    return Err("Truncated Mach-O dylib command".into());
                }
                let name = read_u32_le(slice, command + 8).unwrap() as usize;
                if name < 24 || name >= size {
                    return Err("Invalid Mach-O dylib name offset".into());
                }
                libraries.push(
                    cstring(&slice[command..command + size], name)
                        .ok_or("Invalid Mach-O dylib name")?,
                );
            }
            0x20 => return Ok(bindings), // Legacy lazy-load ordinals not implemented.
            0x19 => {
                if size < 72 {
                    return Err("Truncated import segment".into());
                }
                let count = read_u32_le(slice, command + 64).unwrap() as usize;
                validate_table(command + 72, count, 80, 80, command + size)?;
                for n in 0..count {
                    let at = command + 72 + n * 80;
                    let kind = read_u32_le(slice, at + 64).unwrap() & 0xff;
                    let name = clean_fixed_str(&slice[at..at + 16]);
                    if !matches!(kind, 6..=8)
                        && !matches!(name.as_str(), "__objc_stubs" | "__objc_selrefs")
                    {
                        continue;
                    }
                    let address = read_u64_le(slice, at + 32).unwrap();
                    let length = usize::try_from(read_u64_le(slice, at + 40).unwrap())
                        .map_err(|_| "Import section too large")?;
                    let offset = read_u32_le(slice, at + 48).unwrap() as usize;
                    checked_range(offset, length, slice.len())?;
                    address
                        .checked_add(length as u64)
                        .ok_or("Import section address overflow")?;
                    sections.push(IndirectSection {
                        stride: if name == "__objc_stubs" {
                            32
                        } else if kind == 8 {
                            read_u32_le(slice, at + 72).unwrap() as usize
                        } else {
                            8
                        },
                        name,
                        address,
                        offset,
                        size: length,
                        kind,
                        first: read_u32_le(slice, at + 68).unwrap() as usize,
                    });
                }
            }
            _ => {}
        }
        command += size;
    }
    let (Some((symoff, symcount, stroff, strsize)), Some((indoff, indcount))) = (symbols, indirect)
    else {
        return Ok(bindings);
    };
    let strings = &slice[stroff..stroff + strsize];
    let index = |section: &IndirectSection, number: usize| -> Option<u32> {
        let n = section.first.checked_add(number)?;
        if n >= indcount {
            return None;
        }
        let symbol = read_u32_le(slice, indoff.checked_add(n.checked_mul(4)?)?)?;
        // INDIRECT_SYMBOL_LOCAL / ABS are not indices into nlist_64.
        (symbol & 0xc0000000 == 0 && (symbol as usize) < symcount).then_some(symbol)
    };
    for section in sections.iter().filter(|s| {
        (s.kind == 8 && s.stride == 12 && s.size.is_multiple_of(12))
            || (s.name == "__objc_stubs" && s.kind == 0 && s.size.is_multiple_of(32))
    }) {
        let objc = section.name == "__objc_stubs";
        let code_size = if objc { 20 } else { 12 };
        for n in 0..section.size / section.stride {
            let stub_symbol = if objc {
                None
            } else {
                let Some(symbol) = index(section, n) else {
                    continue;
                };
                Some(symbol)
            };
            let pc = section.address + (n * section.stride) as u64;
            let offset = section.offset + n * section.stride;
            let code = &slice[offset..offset + code_size];
            if !pc.is_multiple_of(4)
                || !segments.iter().any(|s| {
                    s.permissions == Some(5)
                        && s.file_offset_for_address(pc) == Some(offset as u64)
                        && s.read_bytes(pc, code_size) == Some(code)
                })
            {
                continue;
            }
            let selector_address = if objc {
                let a = read_u32_le(code, 0).unwrap();
                let l = read_u32_le(code, 4).unwrap();
                if a & 0x9f00001f != 0x90000001 || l & 0xffc003ff != 0xf9400021 {
                    continue;
                }
                let imm = (((a >> 5) & 0x7ffff) << 2) | ((a >> 29) & 3);
                let pages = ((imm as i64) << 43) >> 43;
                let Some(selref) = (pc & !0xfff)
                    .wrapping_add((pages << 12) as u64)
                    .checked_add((((l >> 10) & 4095) * 8) as u64)
                else {
                    continue;
                };
                let selrefs: Vec<_> = sections
                    .iter()
                    .filter(|s| {
                        s.name == "__objc_selrefs"
                            && selref >= s.address
                            && selref - s.address < s.size as u64
                    })
                    .collect();
                let [s] = selrefs.as_slice() else {
                    continue;
                };
                let rel = selref - s.address;
                if !rel.is_multiple_of(8)
                    || s.size % 8 != 0
                    || !segments.iter().any(|seg| {
                        seg.permissions.is_some_and(|p| p & 1 != 0)
                            && seg.file_offset_for_address(selref) == Some(s.offset as u64 + rel)
                            && seg.read_bytes(selref, 8).is_some()
                    })
                {
                    continue;
                }
                Some(selref)
            } else {
                None
            };
            let suffix = if objc { 8 } else { 0 };
            let a = read_u32_le(code, suffix).unwrap();
            let l = read_u32_le(code, suffix + 4).unwrap();
            let b = read_u32_le(code, suffix + 8).unwrap();
            if a & 0x9f00001f != 0x90000010 || l & 0xffc003ff != 0xf9400210 || b != 0xd61f0200 {
                continue;
            }
            let imm = (((a >> 5) & 0x7ffff) << 2) | ((a >> 29) & 3);
            let pages = ((imm as i64) << 43) >> 43;
            let page = ((pc + suffix as u64) & !0xfff).wrapping_add((pages << 12) as u64);
            let Some(pointer) = page.checked_add((((l >> 10) & 4095) * 8) as u64) else {
                continue;
            };
            if !pointer.is_multiple_of(8) {
                continue;
            }
            let matches: Vec<_> = sections
                .iter()
                .filter(|s| {
                    matches!(s.kind, 6 | 7)
                        && pointer >= s.address
                        && pointer - s.address < s.size as u64
                })
                .collect();
            let [slot] = matches.as_slice() else {
                continue;
            };
            let relative = pointer - slot.address;
            let Some(symbol_index) = index(slot, (relative / 8) as usize) else {
                continue;
            };
            if !relative.is_multiple_of(8)
                || slot.size % 8 != 0
                || stub_symbol.is_some_and(|s| s != symbol_index)
            {
                continue;
            }
            let pointer_bytes =
                slice.get(slot.offset + relative as usize..slot.offset + relative as usize + 8);
            if !segments.iter().any(|s| {
                s.permissions.is_some_and(|p| p & 1 != 0)
                    && s.file_offset_for_address(pointer) == Some(slot.offset as u64 + relative)
                    && s.read_bytes(pointer, 8)
                        .is_some_and(|bytes| Some(bytes) == pointer_bytes)
            }) {
                continue;
            }
            let entry = symoff + symbol_index as usize * 16;
            let desc = read_u16_le(slice, entry + 6).unwrap();
            // Strong external undefined symbol, no common symbol value, and a
            // concrete dependency ordinal. Weak/flat/dynamic imports stay unknown.
            if slice[entry + 4] != 1
                || slice[entry + 5] != 0
                || read_u64_le(slice, entry + 8) != Some(0)
                || desc & 0x40 != 0
            {
                continue;
            }
            let ordinal = (desc >> 8) as usize;
            if ordinal == 0 || ordinal >= 0xfe {
                continue;
            }
            let Some(library) = libraries.get(ordinal - 1) else {
                continue;
            };
            let name_offset = read_u32_le(slice, entry).unwrap() as usize;
            if name_offset == 0 {
                continue;
            }
            let Some(symbol) = cstring(strings, name_offset) else {
                continue;
            };
            if objc && (symbol != "_objc_msgSend" || library != "/usr/lib/libobjc.A.dylib") {
                continue;
            }
            let Some(branch) = pc.checked_add(code_size as u64 - 4) else {
                continue;
            };
            // Overlapping stub declarations are ambiguous, even if names agree.
            if bindings.contains_key(&branch) {
                return Err("Overlapping Mach-O import stubs".into());
            }
            bindings.insert(
                branch,
                MachOImportBinding {
                    library_ordinal: ordinal as u16,
                    context_sha256: context.to_string(),
                    stub_address: pc,
                    pointer_address: pointer,
                    symbol,
                    library: library.clone(),
                    instructions: code
                        .chunks_exact(4)
                        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
                        .collect(),
                    selector_address,
                },
            );
        }
    }
    Ok(bindings)
}
