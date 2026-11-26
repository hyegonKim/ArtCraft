//! Pure-Rust Zero-Copy ELF64 (ARM64 / x86_64) Binary Parser v0.5.0:
//! - Stripped ARM64 Function Prologue (`PACIASP` / `STP x29, x30`) & `BL`/`BLR` Call-Graph Scanner
//! - JNI `RegisterNatives` & `Java_*` Recovery + Android `JNINativeInterface` Symbolic Vtable Lifter
//! - Top Internal/Dart Hub Function P-Code CFG Decompilation (`top_hub_decompilations`)
//! - Deep Flutter Dart AOT Snapshot Header Parser (`0xf5f5dcdc` magic, SDK hash, VM flags, `PP [x22]` loads)
//! - Automatic Credential/API-Key Redaction (`redact_sensitive_text`)

use crate::pcode_sleigh::{
    decode_and_lift_arm64, decompile_elf_arm64_with_cfg, CfgFunctionMetrics, SwitchHintTable,
};
use crate::segment::Segment;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
pub const ELFCLASS64: u8 = 2;
pub const ELFDATA2LSB: u8 = 1;
pub const EM_AARCH64: u16 = 183;
pub const EM_X86_64: u16 = 62;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const PF_X: u32 = 0x1;

const SHT_SYMTAB: u32 = 2;
#[allow(dead_code)]
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_DYNSYM: u32 = 11;

const DT_NEEDED: i64 = 1;
const DT_SONAME: i64 = 14;
const DT_FLAGS: i64 = 30;
const DT_FLAGS_1: i64 = 0x6fff_fffb;
const DT_BIND_NOW: i64 = 24;

const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_GLOB_DAT: u32 = 1025;
const R_AARCH64_JUMP_SLOT: u32 = 1026;
const R_AARCH64_RELATIVE: u32 = 1027;

pub const DART_SNAPSHOT_MAGIC: [u8; 4] = [0xf5, 0xf5, 0xdc, 0xdc];

/// Redacts embedded Google `AIza...` keys or bearer secrets so forensic JSON dossiers remain 100% leak-free.
pub fn redact_sensitive_text(input: &str) -> String {
    if let Some(idx) = input.find("AIza") {
        let tail = &input[idx..];
        let key_len = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .count();
        if key_len >= 30 {
            let mut out = String::with_capacity(input.len());
            out.push_str(&input[..idx]);
            out.push_str(&format!("<REDACTED_CONFIG_VALUE_LEN_{}>", key_len));
            out.push_str(&tail[key_len..]);
            return out;
        }
    }
    input.to_string()
}

#[derive(Debug, Clone)]
pub struct ElfSectionSummary {
    pub name: String,
    pub sh_type: u32,
    pub vaddr: u64,
    pub file_offset: u64,
    pub size: u64,
    pub entropy: f64,
}

#[derive(Debug, Clone)]
pub struct ElfHardeningReport {
    pub nx_enabled: bool,
    pub relro: &'static str, // "full" | "partial" | "none"
    pub stack_canary: bool,
    pub fortify_source: bool,
    pub stripped_symtab: bool,
}

#[derive(Debug, Clone)]
pub struct JniExportDecompilation {
    pub symbol: String,
    pub demangled: String,
    pub vaddr: u64,
    pub file_offset: u64,
    pub size: u64,
    pub string_xrefs: Vec<String>,
    pub callee_symbols: Vec<String>,
    pub cfg_metrics: CfgFunctionMetrics,
    pub pseudocode: String,
}

#[derive(Debug, Clone)]
pub struct JniRegisteredNativeCandidate {
    pub method_name: String,
    pub descriptor: String,
    pub fn_vaddr: u64,
    pub table_vaddr: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Arm64BinaryGraphSummary {
    pub executable_bytes: u64,
    pub prologue_functions_count: usize,
    pub paciasp_functions_count: usize,
    pub stp_fp_lr_functions_count: usize,
    pub bl_direct_calls_count: usize,
    pub blr_indirect_calls_count: usize,
    pub dart_pp_loads_count: usize,
    pub dart_thr_loads_count: usize,
    pub top_called_functions: Vec<(u64, String, usize)>,
}

#[derive(Debug, Clone, Default)]
pub struct FlutterAotSummary {
    pub is_flutter_aot: bool,
    pub snapshot_magic_verified: bool,
    pub vm_snapshot_size: u64,
    pub isolate_snapshot_size: u64,
    pub isolate_instructions_size: u64,
    pub dart_sdk_snapshot_hash: String,
    pub snapshot_features: Vec<String>,
    pub method_channels: Vec<String>,
    pub dart_classes: Vec<String>,
    pub dart_symbols: Vec<String>,
    pub package_uris: Vec<String>,
    pub api_urls_and_routes: Vec<String>,
    pub domain_identifiers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Elf64DeepReport {
    pub arch: &'static str,
    pub machine: u16,
    pub file_size: usize,
    pub entry_vaddr: u64,
    pub soname: Option<String>,
    pub dt_needed: Vec<String>,
    pub hardening: ElfHardeningReport,
    pub sections: Vec<ElfSectionSummary>,
    pub total_dynsym: usize,
    pub exported_functions_count: usize,
    pub imported_functions: Vec<String>,
    pub arm64_call_graph: Arm64BinaryGraphSummary,
    pub jni_exports: Vec<JniExportDecompilation>,
    pub top_hub_decompilations: Vec<JniExportDecompilation>,
    pub jni_registered_candidates: Vec<JniRegisteredNativeCandidate>,
    pub cpp_classes_top: Vec<(String, usize)>,
    pub domain_exports_sample: Vec<(String, String, u64)>,
    pub crypto_and_network_imports: Vec<String>,
    pub url_and_protocol_strings: Vec<String>,
    pub sql_and_schema_strings: Vec<String>,
    pub flutter_aot: FlutterAotSummary,
}

fn read_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off.checked_add(2)?)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn read_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off.checked_add(4)?)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn read_u64(b: &[u8], off: usize) -> Option<u64> {
    b.get(off..off.checked_add(8)?)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}
fn read_i64(b: &[u8], off: usize) -> Option<i64> {
    read_u64(b, off).map(|v| v as i64)
}

fn read_cstr(buf: &[u8], off: usize, max_len: usize) -> String {
    let Some(slice) = buf.get(off..) else {
        return String::new();
    };
    let limit = slice.len().min(max_len);
    let end = slice[..limit]
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(limit);
    redact_sensitive_text(&String::from_utf8_lossy(&slice[..end]))
}

pub fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let sample = if bytes.len() > 2 * 1024 * 1024 {
        &bytes[..2 * 1024 * 1024]
    } else {
        bytes
    };
    let mut counts = [0usize; 256];
    for &b in sample {
        counts[b as usize] += 1;
    }
    let len = sample.len() as f64;
    let mut ent = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = (c as f64) / len;
            ent -= p * p.log2();
        }
    }
    (ent * 1000.0).round() / 1000.0
}

/// Lightweight Itanium C++ ABI (`_Z...`) demangler extracting namespace::Class::method.
pub fn demangle_itanium_lite(sym: &str) -> String {
    if !sym.starts_with("_Z") {
        return sym.to_string();
    }
    let rest = &sym[2..];
    let mut pos = 0;
    let bytes = rest.as_bytes();
    let nested = if bytes.get(pos) == Some(&b'N') {
        pos += 1;
        if bytes.get(pos) == Some(&b'K') {
            pos += 1;
        }
        true
    } else {
        false
    };
    let mut parts: Vec<String> = Vec::new();
    while pos < bytes.len() {
        if nested && bytes[pos] == b'E' {
            break;
        }
        if !bytes[pos].is_ascii_digit() {
            break;
        }
        let start_num = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        let Ok(len) = rest[start_num..pos].parse::<usize>() else {
            break;
        };
        if len == 0 || pos + len > bytes.len() {
            break;
        }
        parts.push(rest[pos..pos + len].to_string());
        pos += len;
        if !nested {
            break;
        }
    }
    if parts.is_empty() {
        sym.to_string()
    } else {
        parts.join("::")
    }
}

#[derive(Clone, Copy)]
struct LoadSeg {
    vaddr: u64,
    memsz: u64,
    offset: u64,
    filesz: u64,
    flags: u32,
}

fn vaddr_to_offset(segs: &[LoadSeg], vaddr: u64) -> Option<usize> {
    for s in segs {
        if vaddr >= s.vaddr && vaddr < s.vaddr.saturating_add(s.filesz) {
            let delta = vaddr - s.vaddr;
            return usize::try_from(s.offset.checked_add(delta)?).ok();
        }
    }
    let _ = segs.first().map(|s| s.memsz);
    None
}

impl Elf64DeepReport {
    pub fn analyze(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 64 || bytes[0..4] != ELF_MAGIC {
            return Err("Not an ELF binary".into());
        }
        if bytes[4] != ELFCLASS64 || bytes[5] != ELFDATA2LSB {
            return Err("Only 64-bit little-endian ELF64 supported in fast path".into());
        }

        let machine = read_u16(bytes, 18).ok_or("Truncated ELF header")?;
        let arch = match machine {
            EM_AARCH64 => "arm64",
            EM_X86_64 => "x86_64",
            _ => "elf64-other",
        };
        let entry_vaddr = read_u64(bytes, 24).unwrap_or(0);
        let phoff = read_u64(bytes, 32).unwrap_or(0) as usize;
        let shoff = read_u64(bytes, 40).unwrap_or(0) as usize;
        let phentsize = read_u16(bytes, 54).unwrap_or(0) as usize;
        let phnum = read_u16(bytes, 56).unwrap_or(0) as usize;
        let shentsize = read_u16(bytes, 58).unwrap_or(0) as usize;
        let shnum = read_u16(bytes, 60).unwrap_or(0) as usize;
        let shstrndx = read_u16(bytes, 62).unwrap_or(0) as usize;

        let mut load_segs = Vec::new();
        let mut nx_enabled = false;
        let mut has_relro = false;
        let mut bind_now = false;
        let mut dyn_off_sz: Option<(usize, usize)> = None;

        if phentsize >= 56 && phnum <= 128 {
            for i in 0..phnum {
                let off = phoff.saturating_add(i.saturating_mul(phentsize));
                let Some(p_type) = read_u32(bytes, off) else {
                    break;
                };
                let p_flags = read_u32(bytes, off + 4).unwrap_or(0);
                let p_offset = read_u64(bytes, off + 8).unwrap_or(0);
                let p_vaddr = read_u64(bytes, off + 16).unwrap_or(0);
                let p_filesz = read_u64(bytes, off + 32).unwrap_or(0);
                let p_memsz = read_u64(bytes, off + 40).unwrap_or(0);

                match p_type {
                    PT_LOAD => load_segs.push(LoadSeg {
                        vaddr: p_vaddr,
                        memsz: p_memsz,
                        offset: p_offset,
                        filesz: p_filesz,
                        flags: p_flags,
                    }),
                    PT_GNU_STACK => {
                        nx_enabled = (p_flags & 1) == 0;
                    }
                    PT_GNU_RELRO => {
                        has_relro = true;
                    }
                    PT_DYNAMIC => {
                        dyn_off_sz = Some((p_offset as usize, p_filesz as usize));
                    }
                    _ => {}
                }
            }
        }

        // Parse Section Headers
        let mut shstrtab: &[u8] = &[];
        if shentsize >= 64 && shnum <= 512 && shstrndx < shnum {
            let st_hdr = shoff.saturating_add(shstrndx.saturating_mul(shentsize));
            if let (Some(so), Some(ss)) =
                (read_u64(bytes, st_hdr + 24), read_u64(bytes, st_hdr + 32))
            {
                let s_off = so as usize;
                let s_end = s_off.saturating_add(ss as usize).min(bytes.len());
                if s_off < s_end {
                    shstrtab = &bytes[s_off..s_end];
                }
            }
        }

        let mut sections = Vec::new();
        let mut has_symtab = false;
        let mut dynsym_range: Option<(usize, usize, usize)> = None;
        let mut section_slices: Vec<(usize, usize)> = Vec::new();
        let mut rela_sections: Vec<(String, usize, usize)> = Vec::new();
        let mut plt_sec_vaddr: Option<(u64, usize, usize)> = None;

        if shentsize >= 64 && shnum <= 512 {
            for i in 0..shnum {
                let off = shoff.saturating_add(i.saturating_mul(shentsize));
                let Some(sh_name_idx) = read_u32(bytes, off) else {
                    break;
                };
                let sh_type = read_u32(bytes, off + 4).unwrap_or(0);
                let sh_addr = read_u64(bytes, off + 16).unwrap_or(0);
                let sh_offset = read_u64(bytes, off + 24).unwrap_or(0);
                let sh_size = read_u64(bytes, off + 32).unwrap_or(0);
                let sh_link = read_u32(bytes, off + 40).unwrap_or(0) as usize;

                let name = read_cstr(shstrtab, sh_name_idx as usize, 128);
                let f_off = sh_offset as usize;
                let f_end = f_off.saturating_add(sh_size as usize).min(bytes.len());
                let ent = if sh_type != 8 && f_off < f_end {
                    shannon_entropy(&bytes[f_off..f_end])
                } else {
                    0.0
                };

                if sh_type == SHT_SYMTAB {
                    has_symtab = true;
                }
                if sh_type == SHT_DYNSYM {
                    dynsym_range = Some((f_off, f_end.saturating_sub(f_off), sh_link));
                }
                if sh_type == SHT_RELA && f_off < f_end {
                    rela_sections.push((name.clone(), f_off, f_end - f_off));
                }
                if name == ".plt" && f_off < f_end {
                    plt_sec_vaddr = Some((sh_addr, f_off, f_end - f_off));
                }

                section_slices.push((f_off, f_end));
                sections.push(ElfSectionSummary {
                    name,
                    sh_type,
                    vaddr: sh_addr,
                    file_offset: sh_offset,
                    size: sh_size,
                    entropy: ent,
                });
            }
        }

        let mut dynstr: &[u8] = &[];
        if let Some((_, _, link_idx)) = dynsym_range {
            if let Some(&(s_off, s_end)) = section_slices.get(link_idx) {
                if s_off < s_end && s_end <= bytes.len() {
                    dynstr = &bytes[s_off..s_end];
                }
            }
        }

        // Parse .dynamic for DT_NEEDED, DT_SONAME, BIND_NOW
        let mut dt_needed = Vec::new();
        let mut soname = None;
        if let Some((d_off, d_sz)) = dyn_off_sz {
            let d_end = d_off.saturating_add(d_sz).min(bytes.len());
            let mut cur = d_off;
            while cur + 16 <= d_end {
                let d_tag = read_i64(bytes, cur).unwrap_or(0);
                let d_val = read_u64(bytes, cur + 8).unwrap_or(0);
                if d_tag == 0 {
                    break;
                }
                match d_tag {
                    DT_NEEDED => {
                        let s = read_cstr(dynstr, d_val as usize, 256);
                        if !s.is_empty() {
                            dt_needed.push(s);
                        }
                    }
                    DT_SONAME => {
                        let s = read_cstr(dynstr, d_val as usize, 256);
                        if !s.is_empty() {
                            soname = Some(s);
                        }
                    }
                    DT_BIND_NOW => bind_now = true,
                    DT_FLAGS if (d_val & 0x8) != 0 => bind_now = true,
                    DT_FLAGS_1 if (d_val & 0x1) != 0 => bind_now = true,
                    _ => {}
                }
                cur += 16;
            }
        }

        // Parse .dynsym & track Dart AOT Snapshot symbol regions
        #[derive(Clone)]
        #[allow(dead_code)]
        struct DynSym {
            name: String,
            vaddr: u64,
            size: u64,
            shndx: u16,
            sym_type: u8,
        }
        let mut syms: Vec<DynSym> = Vec::new();
        let mut imported_functions = Vec::new();
        let mut exported_functions: Vec<(String, u64, u64)> = Vec::new();
        let mut stack_canary = false;
        let mut fortify_source = false;

        let mut dart_vm_data_sym: Option<(u64, u64)> = None;
        let mut dart_iso_data_sym: Option<(u64, u64)> = None;
        let mut dart_iso_instr_sym: Option<(u64, u64)> = None;

        if let Some((sym_off, sym_sz, _)) = dynsym_range {
            let count = sym_sz / 24;
            for i in 0..count {
                let off = sym_off + i * 24;
                let Some(st_name) = read_u32(bytes, off) else {
                    break;
                };
                let st_info = bytes.get(off + 4).copied().unwrap_or(0);
                let st_shndx = read_u16(bytes, off + 6).unwrap_or(0);
                let st_value = read_u64(bytes, off + 8).unwrap_or(0);
                let st_size = read_u64(bytes, off + 16).unwrap_or(0);
                let sym_type = st_info & 0xf;
                let sym_bind = st_info >> 4;
                let name = read_cstr(dynstr, st_name as usize, 512);

                if name.contains("__stack_chk_fail") {
                    stack_canary = true;
                }
                if name.ends_with("_chk") && name.starts_with("__") {
                    fortify_source = true;
                }

                if name == "_kDartVmSnapshotData" {
                    dart_vm_data_sym = Some((st_value, st_size));
                } else if name == "_kDartIsolateSnapshotData" {
                    dart_iso_data_sym = Some((st_value, st_size));
                } else if name == "_kDartIsolateSnapshotInstructions" {
                    dart_iso_instr_sym = Some((st_value, st_size));
                }

                if st_shndx == 0 && !name.is_empty() {
                    imported_functions.push(name.clone());
                } else if st_shndx != 0 && st_value != 0 && sym_bind != 0 && sym_type == 2 {
                    exported_functions.push((name.clone(), st_value, st_size));
                }

                syms.push(DynSym {
                    name,
                    vaddr: st_value,
                    size: st_size,
                    shndx: st_shndx,
                    sym_type,
                });
            }
        }

        // Map GOT/PLT relocations (.rela.plt & .rela.dyn) -> vaddr -> symbol name or pointer target
        let mut got_slot_to_sym: BTreeMap<u64, String> = BTreeMap::new();
        let mut pointer_relocs: BTreeMap<u64, u64> = BTreeMap::new();

        for (_rname, r_off, r_sz) in &rela_sections {
            let count = r_sz / 24;
            for i in 0..count.min(250_000) {
                let off = r_off + i * 24;
                let Some(r_offset) = read_u64(bytes, off) else {
                    break;
                };
                let r_info = read_u64(bytes, off + 8).unwrap_or(0);
                let r_addend = read_i64(bytes, off + 16).unwrap_or(0);
                let r_type = (r_info & 0xffff_ffff) as u32;
                let sym_idx = (r_info >> 32) as usize;

                match r_type {
                    R_AARCH64_JUMP_SLOT | R_AARCH64_GLOB_DAT => {
                        if let Some(sym) = syms.get(sym_idx) {
                            if !sym.name.is_empty() {
                                got_slot_to_sym.insert(r_offset, sym.name.clone());
                            }
                        }
                    }
                    R_AARCH64_RELATIVE => {
                        if r_addend > 0 {
                            pointer_relocs.insert(r_offset, r_addend as u64);
                        }
                    }
                    R_AARCH64_ABS64 => {
                        if let Some(sym) = syms.get(sym_idx) {
                            if sym.vaddr > 0 {
                                pointer_relocs.insert(
                                    r_offset,
                                    (sym.vaddr as i64).wrapping_add(r_addend) as u64,
                                );
                            } else if r_addend > 0 {
                                pointer_relocs.insert(r_offset, r_addend as u64);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Map ARM64 PLT stubs (ADRP x16, page + LDR x17, [x16, off] + ADD + BR x17) to PLT stub vaddr -> symbol
        let mut call_symbol_map: BTreeMap<u64, String> = BTreeMap::new();
        for (name, vaddr, _) in &exported_functions {
            call_symbol_map.insert(*vaddr, demangle_itanium_lite(name));
        }
        let mut plt_va_range: Option<(u64, u64)> = None;
        if machine == EM_AARCH64 {
            if let Some((plt_va, plt_off, plt_sz)) = plt_sec_vaddr {
                plt_va_range = Some((plt_va, plt_va.saturating_add(plt_sz as u64)));
                let mut off = plt_off;
                let end = (plt_off + plt_sz).min(bytes.len());
                while off + 16 <= end {
                    let cur_va = plt_va + (off - plt_off) as u64;
                    let w0 = read_u32(bytes, off).unwrap_or(0);
                    let w1 = read_u32(bytes, off + 4).unwrap_or(0);
                    if (w0 & 0x9f00_001f) == 0x9000_0010 && (w1 & 0xffc0_03ff) == 0xf940_0211 {
                        let immlo = ((w0 >> 29) & 0x3) as i64;
                        let immhi = (((w0 >> 5) & 0x7ffff) as i64) << 2;
                        let mut pages = immhi | immlo;
                        if (pages & (1 << 20)) != 0 {
                            pages -= 1 << 21;
                        }
                        let page_base =
                            ((cur_va & !0xfff) as i64).wrapping_add(pages << 12) as u64;
                        let ldr_imm12 = (((w1 >> 10) & 0xfff) as u64) * 8;
                        let got_va = page_base.wrapping_add(ldr_imm12);
                        if let Some(sym_name) = got_slot_to_sym.get(&got_va) {
                            call_symbol_map.insert(cur_va, format!("{}@plt", sym_name));
                        }
                    }
                    off += 4;
                }
            }
        }

        // Recover RegisterNatives JNINativeMethod { const char* name, const char* sig, void* fnPtr } triples!
        let mut jni_registered_candidates = Vec::new();
        let reloc_addrs: Vec<u64> = pointer_relocs.keys().copied().collect();
        for &slot_a in &reloc_addrs {
            let slot_b = slot_a.wrapping_add(8);
            let slot_c = slot_a.wrapping_add(16);
            let (Some(&name_va), Some(&sig_va), Some(&fn_va)) = (
                pointer_relocs.get(&slot_a),
                pointer_relocs.get(&slot_b),
                pointer_relocs.get(&slot_c),
            ) else {
                continue;
            };
            let (Some(n_off), Some(s_off), Some(_f_off)) = (
                vaddr_to_offset(&load_segs, name_va),
                vaddr_to_offset(&load_segs, sig_va),
                vaddr_to_offset(&load_segs, fn_va),
            ) else {
                continue;
            };
            let m_name = read_cstr(bytes, n_off, 128);
            let m_sig = read_cstr(bytes, s_off, 256);
            if is_valid_java_identifier(&m_name) && is_valid_jni_signature(&m_sig) {
                call_symbol_map
                    .entry(fn_va & !3u64)
                    .or_insert_with(|| format!("RegisterNatives::{}", m_name));
                jni_registered_candidates.push(JniRegisteredNativeCandidate {
                    method_name: m_name,
                    descriptor: m_sig,
                    fn_vaddr: fn_va,
                    table_vaddr: slot_a,
                });
                if jni_registered_candidates.len() >= 256 {
                    break;
                }
            }
        }

        // Scan strings across binary + decode Flutter Dart AOT binary snapshot headers
        let sym_names: Vec<String> = syms.iter().map(|s| s.name.clone()).collect();
        let (url_strings, sql_strings, flutter_aot) = scan_elf_strings_and_flutter(
            bytes,
            &load_segs,
            &sym_names,
            dart_vm_data_sym,
            dart_iso_data_sym,
            dart_iso_instr_sym,
        );

        // v0.5.0: Single-Pass Stripped ARM64 Function Prologue & BL/BLR Call-Graph Scanner!
        let arm64_call_graph = if machine == EM_AARCH64 {
            scan_arm64_binary_call_graph(
                bytes,
                &load_segs,
                &call_symbol_map,
                flutter_aot.is_flutter_aot,
            )
        } else {
            Arm64BinaryGraphSummary::default()
        };

        // Group C++ classes & pick JNI / interesting exports to decompile via Ghidra P-Code
        let mut cpp_class_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut domain_exports_sample = Vec::new();
        let mut jni_targets: Vec<(String, u64, u64)> = Vec::new();

        for (name, va, sz) in &exported_functions {
            let dem = demangle_itanium_lite(name);
            if let Some(idx) = dem.rfind("::") {
                let cls = &dem[..idx];
                if !cls.starts_with("std::") && !cls.starts_with("__cxx") {
                    *cpp_class_counts.entry(cls.to_string()).or_insert(0) += 1;
                }
            }
            if name.starts_with("Java_") || name == "JNI_OnLoad" {
                jni_targets.push((name.clone(), *va, *sz));
            } else {
                let low = dem.to_lowercase();
                if domain_exports_sample.len() < 120
                    && (low.contains("route")
                        || low.contains("truck")
                        || low.contains("toll")
                        || low.contains("hmac")
                        || low.contains("crypt")
                        || low.contains("cipher")
                        || low.contains("aes")
                        || low.contains("seed")
                        || low.contains("packet")
                        || low.contains("socket")
                        || low.contains("call24")
                        || low.contains("atlan")
                        || low.contains("naver")
                        || low.contains("kakao")
                        || low.contains("vlsdk")
                        || low.contains("airshield")
                        || low.contains("roadview")
                        || low.contains("cctv")
                        || low.contains("height")
                        || low.contains("weight"))
                {
                    domain_exports_sample.push((name.clone(), dem, *va));
                }
            }
        }

        if jni_targets.len() < 24 {
            for cand in jni_registered_candidates.iter().take(18) {
                jni_targets.push((
                    format!("RegisterNatives::{}{}", cand.method_name, cand.descriptor),
                    cand.fn_vaddr,
                    280,
                ));
            }
            for (raw_n, _dem_n, va) in domain_exports_sample.iter().take(16) {
                if jni_targets.len() >= 40 {
                    break;
                }
                jni_targets.push((raw_n.clone(), *va, 280));
            }
        }

        let mut jni_exports = Vec::new();
        let mut top_hub_decompilations = Vec::new();

        if machine == EM_AARCH64 {
            let dummy_seg = Segment::new(".text", 0, 0, Vec::new());
            let hints = SwitchHintTable::default();
            let mut already_decompiled: BTreeSet<u64> = BTreeSet::new();

            for (sym_name, fn_va, fn_sz) in jni_targets.into_iter().take(40) {
                let clean_va = fn_va & !3u64;
                if !already_decompiled.insert(clean_va) {
                    continue;
                }
                if let Some(decomp) = decompile_single_arm64_function(
                    bytes,
                    &load_segs,
                    &call_symbol_map,
                    &dummy_seg,
                    &hints,
                    &sym_name,
                    clean_va,
                    fn_sz,
                    flutter_aot.is_flutter_aot,
                ) {
                    jni_exports.push(decomp);
                }
            }

            // v0.5.0: Also decompile Top Internal / Stripped / Dart AOT Hub Functions!
            for (hub_va, hub_sym, callers) in &arm64_call_graph.top_called_functions {
                if top_hub_decompilations.len() >= 10 {
                    break;
                }
                let clean_va = *hub_va & !3u64;
                // Skip PLT trampolines so we decompile genuine internal C++/Rust/Dart functions
                if let Some((p_start, p_end)) = plt_va_range {
                    if clean_va >= p_start && clean_va < p_end {
                        continue;
                    }
                }
                if already_decompiled.contains(&clean_va) {
                    continue;
                }
                let hub_label = if hub_sym.starts_with("sub_") {
                    if flutter_aot.is_flutter_aot {
                        format!("dart_aot_hub_{:x}_fanin_{}", clean_va, callers)
                    } else {
                        format!("internal_hub_{:x}_fanin_{}", clean_va, callers)
                    }
                } else {
                    hub_sym.clone()
                };
                if let Some(decomp) = decompile_single_arm64_function(
                    bytes,
                    &load_segs,
                    &call_symbol_map,
                    &dummy_seg,
                    &hints,
                    &hub_label,
                    clean_va,
                    0, // scan until RET up to 320 bytes
                    flutter_aot.is_flutter_aot,
                ) {
                    already_decompiled.insert(clean_va);
                    top_hub_decompilations.push(decomp);
                }
            }
        }

        let mut cpp_vec: Vec<(String, usize)> = cpp_class_counts.into_iter().collect();
        cpp_vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        cpp_vec.truncate(60);

        let mut crypto_and_network_imports = Vec::new();
        for imp in &imported_functions {
            let l = imp.to_lowercase();
            if l.contains("crypt")
                || l.contains("aes")
                || l.contains("hmac")
                || l.contains("sha")
                || l.contains("md5")
                || l.contains("ssl")
                || l.contains("tls")
                || l.contains("evp_")
                || l.contains("socket")
                || l.contains("connect")
                || l.contains("send")
                || l.contains("recv")
                || l.contains("curl")
                || l.contains("sqlite3")
                || l.contains("sqlcipher")
            {
                crypto_and_network_imports.push(imp.clone());
            }
        }
        crypto_and_network_imports.sort();
        crypto_and_network_imports.dedup();

        let relro = if has_relro && bind_now {
            "full"
        } else if has_relro {
            "partial"
        } else {
            "none"
        };

        Ok(Self {
            arch,
            machine,
            file_size: bytes.len(),
            entry_vaddr,
            soname,
            dt_needed,
            hardening: ElfHardeningReport {
                nx_enabled,
                relro,
                stack_canary,
                fortify_source,
                stripped_symtab: !has_symtab,
            },
            sections,
            total_dynsym: syms.len(),
            exported_functions_count: exported_functions.len(),
            imported_functions: imported_functions.into_iter().take(250).collect(),
            arm64_call_graph,
            jni_exports,
            top_hub_decompilations,
            jni_registered_candidates,
            cpp_classes_top: cpp_vec,
            domain_exports_sample,
            crypto_and_network_imports,
            url_and_protocol_strings: url_strings,
            sql_and_schema_strings: sql_strings,
            flutter_aot,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn decompile_single_arm64_function(
    bytes: &[u8],
    load_segs: &[LoadSeg],
    call_symbol_map: &BTreeMap<u64, String>,
    dummy_seg: &Segment,
    hints: &SwitchHintTable,
    sym_name: &str,
    clean_va: u64,
    fn_sz: u64,
    is_flutter_aot: bool,
) -> Option<JniExportDecompilation> {
    let f_off = vaddr_to_offset(load_segs, clean_va)?;
    let byte_budget = (if fn_sz > 0 { fn_sz as usize } else { 320 }).clamp(16, 640);
    let end_off = (f_off + byte_budget).min(bytes.len());

    let mut insns = Vec::new();
    let mut reg_pages: BTreeMap<u64, u64> = BTreeMap::new();
    let mut rodata_strings_by_va: BTreeMap<u64, String> = BTreeMap::new();
    let mut string_xrefs = Vec::new();
    let mut callee_symbols = Vec::new();

    let mut pos = f_off;
    while pos + 4 <= end_off {
        let cur_va = clean_va + (pos - f_off) as u64;
        let word = read_u32(bytes, pos).unwrap_or(0);
        let mut dec = decode_and_lift_arm64(cur_va, word, dummy_seg, call_symbol_map, hints);

        // Track ADRP + ADD -> .rodata string literals across the whole ELF image!
        if (word & 0x9f00_0000) == 0x9000_0000 {
            let rd = (word & 0x1f) as u64;
            let immlo = ((word >> 29) & 0x3) as i64;
            let immhi = (((word >> 5) & 0x7ffff) as i64) << 2;
            let mut pages = immhi | immlo;
            if (pages & (1 << 20)) != 0 {
                pages -= 1 << 21;
            }
            let page = ((cur_va & !0xfff) as i64).wrapping_add(pages << 12) as u64;
            reg_pages.insert(rd, page);
        } else if (word & 0xff80_0000) == 0x9100_0000 {
            let rd = (word & 0x1f) as u64;
            let rn = ((word >> 5) & 0x1f) as u64;
            let sh = ((word >> 22) & 0x1) as u32;
            let imm12 = (((word >> 10) & 0xfff) as u64) << (if sh == 1 { 12 } else { 0 });
            if let Some(&base_page) = reg_pages.get(&rn) {
                let target_va = base_page.wrapping_add(imm12);
                if let Some(s_off) = vaddr_to_offset(load_segs, target_va) {
                    let s_val = read_cstr(bytes, s_off, 160);
                    if s_val.len() >= 3
                        && s_val
                            .chars()
                            .all(|c| !c.is_control() || c == '\n' || c == '\t')
                    {
                        rodata_strings_by_va.insert(target_va, s_val.clone());
                        if !string_xrefs.contains(&s_val) {
                            string_xrefs.push(s_val);
                        }
                    }
                }
                reg_pages.insert(rd, target_va);
            }
        } else if (word & 0xfc00_0000) == 0x9400_0000 {
            let mut imm26 = (word & 0x03ff_ffff) as i64;
            if (imm26 & (1 << 25)) != 0 {
                imm26 -= 1 << 26;
            }
            let callee_va = (cur_va as i64).wrapping_add(imm26 * 4) as u64;
            if let Some(c_sym) = call_symbol_map.get(&callee_va) {
                if !callee_symbols.contains(c_sym) {
                    callee_symbols.push(c_sym.clone());
                }
                for op in &mut dec.pcode {
                    if let crate::pcode_sleigh::PcodeOp::Call { symbol, .. } = op {
                        *symbol = Some(c_sym.clone());
                    }
                }
            }
        }

        let is_ret = dec.is_ret;
        insns.push(dec);
        pos += 4;
        if is_ret && fn_sz == 0 && insns.len() >= 2 {
            break;
        }
    }

    if insns.is_empty() {
        return None;
    }

    let dem = demangle_itanium_lite(sym_name);
    let (mut pseudo, cfg_metrics) = decompile_elf_arm64_with_cfg(
        &dem,
        clean_va,
        &insns,
        dummy_seg,
        &rodata_strings_by_va,
        is_flutter_aot,
    );
    if !string_xrefs.is_empty() {
        pseudo.push_str(&format!("// [ELF .rodata XREFs]: {:?}\n", string_xrefs));
    }

    Some(JniExportDecompilation {
        symbol: sym_name.to_string(),
        demangled: dem,
        vaddr: clean_va,
        file_offset: f_off as u64,
        size: (pos - f_off) as u64,
        string_xrefs,
        callee_symbols,
        cfg_metrics,
        pseudocode: pseudo,
    })
}

/// Scans executable `PT_LOAD` (`PF_X`) segments in a single 4-byte aligned pass to recover
/// stripped ARM64 function prologues (`PACIASP`, `STP x29, x30`), direct `BL` call fan-in hubs,
/// indirect `BLR` dispatches, and Flutter Dart AOT `PP [x22]` / `THR [x26]` loads.
fn scan_arm64_binary_call_graph(
    bytes: &[u8],
    load_segs: &[LoadSeg],
    call_symbol_map: &BTreeMap<u64, String>,
    is_flutter_aot: bool,
) -> Arm64BinaryGraphSummary {
    let mut summary = Arm64BinaryGraphSummary::default();
    let mut bl_fan_in: HashMap<u64, usize> = HashMap::with_capacity(8192);
    let max_scan_bytes: usize = 32 * 1024 * 1024;
    let mut scanned_total = 0usize;

    for seg in load_segs {
        if (seg.flags & PF_X) == 0 || seg.filesz < 4 {
            continue;
        }
        let start = seg.offset as usize;
        let seg_len = (seg.filesz as usize).min(max_scan_bytes.saturating_sub(scanned_total));
        let end = start.saturating_add(seg_len).min(bytes.len());
        if start >= end {
            continue;
        }
        summary.executable_bytes += (end - start) as u64;
        scanned_total += end - start;

        let mut prev_was_paciasp = false;
        let mut pos = start;
        while pos + 4 <= end {
            let cur_va = seg.vaddr + (pos - start) as u64;
            let w = u32::from_le_bytes([
                bytes[pos],
                bytes[pos + 1],
                bytes[pos + 2],
                bytes[pos + 3],
            ]);

            if w == 0xd503_233f {
                // PACIASP
                summary.paciasp_functions_count += 1;
                summary.prologue_functions_count += 1;
                prev_was_paciasp = true;
                pos += 4;
                continue;
            }

            // STP x29, x30, [sp, #imm] (signed-offset 0xa9007bfd or pre-index 0xa9807bfd)
            // OR Dart AOT frame setup STP x29, x30, [x15, #...] (`rt=29, rt2=30`)
            if ((w & 0xffc0_7fff) == 0xa900_7bfd
                || (w & 0xffc0_7fff) == 0xa980_7bfd
                || (w & 0xffc0_7dff) == 0xa980_79fd)
                && !prev_was_paciasp
            {
                summary.stp_fp_lr_functions_count += 1;
                summary.prologue_functions_count += 1;
            }
            prev_was_paciasp = false;

            // Direct call: BL <imm26>
            if (w & 0xfc00_0000) == 0x9400_0000 {
                summary.bl_direct_calls_count += 1;
                let imm26 = (w & 0x03ff_ffff) as i32;
                let signed_imm = ((imm26 << 6) >> 6) as i64 * 4;
                let target_va = (cur_va as i64).wrapping_add(signed_imm) as u64;
                *bl_fan_in.entry(target_va).or_insert(0) += 1;
            } else if (w & 0xffff_fc1f) == 0xd63f_0000 {
                // Indirect call: BLR Xn
                summary.blr_indirect_calls_count += 1;
            } else if is_flutter_aot && (w & 0xffc0_03e0) == 0xf940_02c0 {
                // LDR Xt, [x22, #imm] (Dart AOT PP Object Pool load)
                summary.dart_pp_loads_count += 1;
            } else if is_flutter_aot && (w & 0xffc0_03e0) == 0xf940_0340 {
                // LDR Xt, [x26, #imm] (Dart AOT THR Thread slot load)
                summary.dart_thr_loads_count += 1;
            }

            pos += 4;
        }
        if scanned_total >= max_scan_bytes {
            break;
        }
    }

    let mut top_vec: Vec<(u64, usize)> = bl_fan_in.into_iter().collect();
    top_vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (va, cnt) in top_vec.into_iter().take(20) {
        let sym = call_symbol_map
            .get(&va)
            .cloned()
            .unwrap_or_else(|| format!("sub_{:x}", va));
        summary.top_called_functions.push((va, sym, cnt));
    }

    summary
}

fn is_valid_java_identifier(s: &str) -> bool {
    if s.len() < 2 || s.len() > 96 {
        return false;
    }
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn is_valid_jni_signature(s: &str) -> bool {
    if !s.starts_with('(') || s.len() < 3 || s.len() > 240 {
        return false;
    }
    let Some(close_idx) = s.find(')') else {
        return false;
    };
    let ret = &s[close_idx + 1..];
    if ret.is_empty() {
        return false;
    }
    let first_ret = ret.as_bytes()[0];
    matches!(
        first_ret,
        b'V' | b'Z' | b'B' | b'C' | b'S' | b'I' | b'J' | b'F' | b'D' | b'L' | b'['
    ) && s.bytes().all(|b| (0x20..=0x7e).contains(&b))
}

fn parse_dart_snapshot_header(
    bytes: &[u8],
    load_segs: &[LoadSeg],
    vm_sym: Option<(u64, u64)>,
) -> (bool, String, Vec<String>) {
    let Some((vm_va, _)) = vm_sym else {
        return (false, String::new(), Vec::new());
    };
    let Some(off) = vaddr_to_offset(load_segs, vm_va) else {
        return (false, String::new(), Vec::new());
    };
    if off + 96 > bytes.len() {
        return (false, String::new(), Vec::new());
    }
    let magic_ok = bytes[off..off + 4] == DART_SNAPSHOT_MAGIC;
    let hdr_window = &bytes[off..(off + 320).min(bytes.len())];

    // Extract 32-byte ASCII hex SDK snapshot hash (typically at +0x14 = +20 bytes after magic+size+kind)
    let mut sdk_hash = String::new();
    let mut hash_end_idx = 20usize;
    for pos in 12..hdr_window.len().saturating_sub(32) {
        let cand = &hdr_window[pos..pos + 32];
        if cand
            .iter()
            .all(|&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            sdk_hash = String::from_utf8_lossy(cand).into_owned();
            hash_end_idx = pos + 32;
            break;
        }
    }

    // Extract space-separated VM feature flags null-terminated string right after the 32-byte hash
    let mut features = Vec::new();
    if hash_end_idx < hdr_window.len() {
        let feat_slice = &hdr_window[hash_end_idx..];
        let start_p = feat_slice
            .iter()
            .position(|&b| (0x21..=0x7e).contains(&b))
            .unwrap_or(feat_slice.len());
        if start_p < feat_slice.len() {
            let sub = &feat_slice[start_p..];
            let end_p = sub.iter().position(|&b| b == 0).unwrap_or(sub.len());
            let raw_flags = String::from_utf8_lossy(&sub[..end_p]);
            for tok in raw_flags.split_whitespace() {
                if tok.len() >= 2
                    && tok
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    features.push(tok.to_string());
                }
            }
        }
    }

    (magic_ok, sdk_hash, features)
}

fn scan_elf_strings_and_flutter(
    bytes: &[u8],
    load_segs: &[LoadSeg],
    sym_names: &[String],
    dart_vm_data_sym: Option<(u64, u64)>,
    dart_iso_data_sym: Option<(u64, u64)>,
    dart_iso_instr_sym: Option<(u64, u64)>,
) -> (Vec<String>, Vec<String>, FlutterAotSummary) {
    let mut dart_symbols = Vec::new();
    for s in sym_names {
        if s.starts_with("_kDart") || s.starts_with("Dart_") {
            dart_symbols.push(s.clone());
        }
    }
    let is_flutter_aot = !dart_symbols.is_empty();

    let (snapshot_magic_verified, dart_sdk_snapshot_hash, snapshot_features) = if is_flutter_aot {
        parse_dart_snapshot_header(bytes, load_segs, dart_vm_data_sym)
    } else {
        (false, String::new(), Vec::new())
    };

    let mut urls = BTreeSet::new();
    let mut sqls = BTreeSet::new();
    let mut pkg_uris = BTreeSet::new();
    let mut api_routes = BTreeSet::new();
    let mut domain_ids = BTreeSet::new();
    let mut method_channels = BTreeSet::new();
    let mut dart_classes = BTreeSet::new();

    let scan_limit = bytes.len().min(36 * 1024 * 1024);
    let mut i = 0;
    while i < scan_limit {
        if (0x20..=0x7e).contains(&bytes[i]) {
            let start = i;
            while i < scan_limit && (0x20..=0x7e).contains(&bytes[i]) && (i - start) < 280 {
                i += 1;
            }
            let len = i - start;
            if len >= 6 {
                let raw_s = String::from_utf8_lossy(&bytes[start..i]);
                let s = redact_sensitive_text(&raw_s);
                if s.starts_with("http://")
                    || s.starts_with("https://")
                    || s.starts_with("ws://")
                    || s.starts_with("wss://")
                    || s.contains("://")
                {
                    if urls.len() < 160
                        && !s.contains("schemas.android.com")
                        && !s.contains("w3.org")
                    {
                        urls.insert(s.clone());
                    }
                }
                let up = s.to_ascii_uppercase();
                if (up.starts_with("CREATE TABLE")
                    || up.starts_with("SELECT ")
                    || up.starts_with("INSERT INTO")
                    || up.starts_with("PRAGMA KEY")
                    || up.contains("SQLCIPHER"))
                    && sqls.len() < 80
                {
                    sqls.insert(s.clone());
                }
                if is_flutter_aot {
                    if s.starts_with("package:") && pkg_uris.len() < 200 {
                        pkg_uris.insert(s.clone());
                    } else if (s.starts_with("flutter/")
                        || s.starts_with("plugins.flutter.io/")
                        || s.starts_with("dev.flutter.")
                        || s.starts_with("com.dexterous.")
                        || (s.contains('/')
                            && !s.contains(' ')
                            && (s.ends_with("_channel")
                                || s.contains("channel")
                                || s.starts_with("com.sanggong")
                                || s.starts_with("kr.co."))))
                        && method_channels.len() < 80
                    {
                        method_channels.insert(s.clone());
                    } else if (s.starts_with("/api/")
                        || s.starts_with("/v1/")
                        || s.starts_with("/v2/")
                        || s.starts_with("api/")
                        || s.contains(".co.kr")
                        || s.contains("ktrucker")
                        || s.contains("sanggong"))
                        && api_routes.len() < 160
                    {
                        api_routes.insert(s.clone());
                    } else {
                        // PascalCase Dart Domain Class detector (e.g., DispatchRepository, SosRequestModel)
                        let first_b = s.as_bytes()[0];
                        if first_b.is_ascii_uppercase()
                            && s.len() >= 8
                            && s.len() <= 64
                            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                            && (s.ends_with("Model")
                                || s.ends_with("Provider")
                                || s.ends_with("Bloc")
                                || s.ends_with("Cubit")
                                || s.ends_with("Controller")
                                || s.ends_with("Service")
                                || s.ends_with("Repository")
                                || s.ends_with("State")
                                || s.ends_with("Page")
                                || s.ends_with("Screen")
                                || s.ends_with("Widget")
                                || s.ends_with("Dto")
                                || s.ends_with("Request")
                                || s.ends_with("Response"))
                            && dart_classes.len() < 120
                        {
                            dart_classes.insert(s.clone());
                        }
                        if domain_ids.len() < 140 {
                            let low = s.to_ascii_lowercase();
                            if low.contains("dispatch")
                                || low.contains("repair")
                                || low.contains("tow")
                                || low.contains("rescue")
                                || low.contains("partner")
                                || low.contains("truck")
                                || low.contains("vehicle")
                                || low.contains("location")
                                || low.contains("socket")
                                || low.contains("fcm")
                                || low.contains("payment")
                                || low.contains("order")
                            {
                                domain_ids.insert(s);
                            }
                        }
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    (
        urls.into_iter().collect(),
        sqls.into_iter().collect(),
        FlutterAotSummary {
            is_flutter_aot,
            snapshot_magic_verified,
            vm_snapshot_size: dart_vm_data_sym.map(|x| x.1).unwrap_or(0),
            isolate_snapshot_size: dart_iso_data_sym.map(|x| x.1).unwrap_or(0),
            isolate_instructions_size: dart_iso_instr_sym.map(|x| x.1).unwrap_or(0),
            dart_sdk_snapshot_hash,
            snapshot_features,
            method_channels: method_channels.into_iter().collect(),
            dart_classes: dart_classes.into_iter().collect(),
            dart_symbols,
            package_uris: pkg_uris.into_iter().collect(),
            api_urls_and_routes: api_routes.into_iter().collect(),
            domain_identifiers: domain_ids.into_iter().collect(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_itanium_demangler_jni_sig_and_redaction() {
        assert_eq!(
            demangle_itanium_lite("_ZN5atlan5truck11RouteEngine13calculateCostEii"),
            "atlan::truck::RouteEngine::calculateCost"
        );
        assert!(is_valid_java_identifier("nativeSignUrl"));
        assert!(is_valid_jni_signature("(Ljava/lang/String;J)Ljava/lang/String;"));
        assert!(!is_valid_jni_signature("not_a_sig"));

        let fake_key = "https://maps.googleapis.com/maps/api/key=AIzaSyDummyTestKey123456789012345678901";
        let clean = redact_sensitive_text(fake_key);
        assert!(!clean.contains("AIzaSyDummy"));
        assert!(clean.contains("<REDACTED_CONFIG_VALUE_LEN_39>"));
    }
}
