//! Pure-Rust Zero-Copy Android Dalvik Multi-DEX (`035`..`041`) Deep Analyzer v0.5.0:
//! - Annotation Decoder (`@GET/@POST`, `@SerializedName`, `@Entity/@Query`, `@JavascriptInterface`)
//! - `ACC_NATIVE` JNI Signature Extractor
//! - Register-Tracked Dalvik -> Structured Java Decompiler (`decompiled_java` + `cfg_blocks` + `branch_count`)
//! - OkHttp `Interceptor` / `HttpURLConnection` Request Header Chain Extractor (`okhttp_header_chains`)
//! - Android `Intent` / Broadcast Action IPC Contract Extractor (`intent_ipc_contracts`)

use crate::elf_arm64::redact_sensitive_text;
use std::collections::{BTreeMap, BTreeSet};

const ACC_PUBLIC: u32 = 0x0001;
const ACC_STATIC: u32 = 0x0008;
const ACC_NATIVE: u32 = 0x0100;

#[derive(Debug, Clone)]
pub struct DexNativeMethodDecl {
    pub class_name: String,
    pub method_name: String,
    pub jni_descriptor: String,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct DexHttpEndpoint {
    pub http_method: String,
    pub path: String,
    pub host_class: String,
    pub java_method: String,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub struct DexRoomQuery {
    pub dao_class: String,
    pub method_name: String,
    pub annotation_kind: String,
    pub sql_or_table: String,
}

#[derive(Debug, Clone)]
pub struct DexJsBridgeMethod {
    pub bridge_class: String,
    pub method_name: String,
    pub signature: String,
    pub strings_in_body: Vec<String>,
    pub calls_in_body: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DexHeaderChain {
    pub class_name: String,
    pub method_name: String,
    pub header_apis: Vec<String>,
    pub header_keys_and_values: Vec<String>,
    pub decompiled_snippet: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DexDisassembledMethod {
    pub class_name: String,
    pub method_name: String,
    pub signature: String,
    pub category: &'static str, // "crypto" | "network" | "database" | "truck_routing" | "deeplink" | "domain"
    pub insn_units: usize,
    pub cfg_blocks: usize,
    pub branch_count: usize,
    pub const_strings: Vec<String>,
    pub invoked_methods: Vec<String>,
    pub accessed_fields: Vec<String>,
    pub bytecode_preview: Vec<String>,
    pub decompiled_java: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MultiDexDeepReport {
    pub dex_files_scanned: usize,
    pub total_strings: usize,
    pub total_types: usize,
    pub total_methods: usize,
    pub total_fields: usize,
    pub total_classes: usize,
    pub native_declarations: Vec<DexNativeMethodDecl>,
    pub retrofit_endpoints: Vec<DexHttpEndpoint>,
    pub room_entities_and_queries: Vec<DexRoomQuery>,
    pub js_bridge_methods: Vec<DexJsBridgeMethod>,
    pub okhttp_header_chains: Vec<DexHeaderChain>,
    pub intent_ipc_contracts: BTreeSet<String>,
    pub dto_serialized_fields: Vec<(String, String, String)>, // (class, field, json_key)
    pub truck_and_routing_classes: Vec<(String, Vec<String>)>,
    pub crypto_methods: Vec<DexDisassembledMethod>,
    pub network_and_socket_methods: Vec<DexDisassembledMethod>,
    pub database_and_pref_methods: Vec<DexDisassembledMethod>,
    pub domain_core_methods: Vec<DexDisassembledMethod>,
    pub discovered_urls: BTreeSet<String>,
    pub sql_statements: BTreeSet<String>,
    pub shared_prefs_names: BTreeSet<String>,
}

fn r_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off.checked_add(2)?)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn r_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off.checked_add(4)?)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn r_uleb128(b: &[u8], mut off: usize) -> Option<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0u32;
    for _ in 0..5 {
        let byte = *b.get(off)?;
        off += 1;
        result |= ((byte & 0x7f) as u32) << shift;
        if (byte & 0x80) == 0 {
            return Some((result, off));
        }
        shift += 7;
    }
    None
}

fn skip_encoded_value(b: &[u8], off: usize) -> Option<usize> {
    let hdr = *b.get(off)?;
    let val_type = hdr & 0x1f;
    let val_arg = (hdr >> 5) as usize;
    let cur = off + 1;
    match val_type {
        0x00 => cur.checked_add(1),
        0x02 | 0x03 | 0x04 | 0x06 | 0x10 | 0x11 | 0x15 | 0x16 | 0x17 | 0x18 | 0x19 | 0x1a
        | 0x1b => cur.checked_add(val_arg + 1),
        0x1c => {
            let (sz, mut p) = r_uleb128(b, cur)?;
            for _ in 0..sz.min(512) {
                p = skip_encoded_value(b, p)?;
            }
            Some(p)
        }
        0x1d => {
            let (_t, mut p) = r_uleb128(b, cur)?;
            let (sz, mut p2) = r_uleb128(b, p)?;
            p = p2;
            for _ in 0..sz.min(128) {
                let (_name_idx, p_val) = r_uleb128(b, p)?;
                p2 = skip_encoded_value(b, p_val)?;
                p = p2;
            }
            Some(p)
        }
        0x1e | 0x1f => Some(cur),
        _ => None,
    }
}

fn read_encoded_string_or_array(
    b: &[u8],
    off: usize,
    strings: &[String],
) -> Option<(Vec<String>, usize)> {
    let hdr = *b.get(off)?;
    let val_type = hdr & 0x1f;
    let val_arg = (hdr >> 5) as usize;
    let cur = off + 1;
    if val_type == 0x17 {
        let byte_len = val_arg + 1;
        if byte_len > 4 || cur + byte_len > b.len() {
            return None;
        }
        let mut idx = 0usize;
        for i in 0..byte_len {
            idx |= (b[cur + i] as usize) << (i * 8);
        }
        let s = strings.get(idx).cloned().unwrap_or_default();
        return Some((vec![s], cur + byte_len));
    } else if val_type == 0x1c {
        let (sz, mut p) = r_uleb128(b, cur)?;
        let mut out = Vec::new();
        for _ in 0..sz.min(64) {
            if let Some((mut sub, next_p)) = read_encoded_string_or_array(b, p, strings) {
                out.append(&mut sub);
                p = next_p;
            } else {
                p = skip_encoded_value(b, p)?;
            }
        }
        return Some((out, p));
    }
    let next = skip_encoded_value(b, off)?;
    Some((Vec::new(), next))
}

fn desc_to_dotted(desc: &str) -> String {
    if desc.starts_with('L') && desc.ends_with(';') && desc.len() > 2 {
        desc[1..desc.len() - 1].replace('/', ".")
    } else {
        desc.to_string()
    }
}

fn short_class_name(dotted: &str) -> &str {
    dotted.rsplit('.').next().unwrap_or(dotted)
}

impl MultiDexDeepReport {
    pub fn scan_dex_bytes(&mut self, raw: &[u8], app_pkg_prefix: &str) -> Result<(), String> {
        if raw.len() < 112 || &raw[0..4] != b"dex\n" {
            return Err("Invalid DEX header".into());
        }

        let string_ids_size = r_u32(raw, 56).unwrap_or(0) as usize;
        let string_ids_off = r_u32(raw, 60).unwrap_or(0) as usize;
        let type_ids_size = r_u32(raw, 64).unwrap_or(0) as usize;
        let type_ids_off = r_u32(raw, 68).unwrap_or(0) as usize;
        let proto_ids_size = r_u32(raw, 72).unwrap_or(0) as usize;
        let proto_ids_off = r_u32(raw, 76).unwrap_or(0) as usize;
        let field_ids_size = r_u32(raw, 80).unwrap_or(0) as usize;
        let field_ids_off = r_u32(raw, 84).unwrap_or(0) as usize;
        let method_ids_size = r_u32(raw, 88).unwrap_or(0) as usize;
        let method_ids_off = r_u32(raw, 92).unwrap_or(0) as usize;
        let class_defs_size = r_u32(raw, 96).unwrap_or(0) as usize;
        let class_defs_off = r_u32(raw, 100).unwrap_or(0) as usize;

        self.dex_files_scanned += 1;
        self.total_strings += string_ids_size;
        self.total_types += type_ids_size;
        self.total_fields += field_ids_size;
        self.total_methods += method_ids_size;
        self.total_classes += class_defs_size;

        // 1. Read Strings & harvest URLs + SQL (with automatic credential redaction)
        let mut strings = Vec::with_capacity(string_ids_size.min(500_000));
        for i in 0..string_ids_size {
            let Some(s_off) = r_u32(raw, string_ids_off + i * 4) else {
                strings.push(String::new());
                continue;
            };
            let Some((_utf16_len, pos)) = r_uleb128(raw, s_off as usize) else {
                strings.push(String::new());
                continue;
            };
            let slice = raw.get(pos..).unwrap_or(&[]);
            let end = slice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(slice.len().min(1024));
            let s = redact_sensitive_text(&String::from_utf8_lossy(&slice[..end]));

            if s.len() >= 8 && s.len() <= 260 {
                if (s.starts_with("http://")
                    || s.starts_with("https://")
                    || s.starts_with("ws://")
                    || s.starts_with("wss://")
                    || s.contains("://"))
                    && !s.contains("schemas.android.com")
                    && !s.contains("www.w3.org")
                    && !s.contains("apache.org")
                    && self.discovered_urls.len() < 400
                {
                    self.discovered_urls.insert(s.clone());
                }
                let up = s.to_ascii_uppercase();
                if (up.starts_with("CREATE TABLE")
                    || up.starts_with("CREATE UNIQUE INDEX")
                    || up.starts_with("SELECT ")
                    || up.starts_with("INSERT OR REPLACE INTO")
                    || up.starts_with("ALTER TABLE "))
                    && self.sql_statements.len() < 200
                {
                    self.sql_statements.insert(s.clone());
                }
            }
            strings.push(s);
        }

        // 2. Read Types
        let mut types = Vec::with_capacity(type_ids_size.min(200_000));
        for i in 0..type_ids_size {
            let idx = r_u32(raw, type_ids_off + i * 4).unwrap_or(u32::MAX) as usize;
            types.push(strings.get(idx).cloned().unwrap_or_default());
        }

        // 3. Read Protos -> JNI Descriptor `(...)Ret`
        let mut protos = Vec::with_capacity(proto_ids_size.min(200_000));
        for i in 0..proto_ids_size {
            let base = proto_ids_off + i * 12;
            let ret_idx = r_u32(raw, base + 4).unwrap_or(u32::MAX) as usize;
            let params_off = r_u32(raw, base + 8).unwrap_or(0) as usize;
            let ret_t = types.get(ret_idx).map(String::as_str).unwrap_or("V");
            let mut desc = String::from("(");
            if params_off != 0 {
                if let Some(cnt) = r_u32(raw, params_off) {
                    for p in 0..(cnt as usize).min(64) {
                        if let Some(tid) = r_u16(raw, params_off + 4 + p * 2) {
                            if let Some(pt) = types.get(tid as usize) {
                                desc.push_str(pt);
                            }
                        }
                    }
                }
            }
            desc.push(')');
            desc.push_str(ret_t);
            protos.push(desc);
        }

        // 4. Read Fields (class_idx, type_idx, name_idx)
        let mut fields: Vec<(u16, u16, u32)> = Vec::with_capacity(field_ids_size.min(300_000));
        for i in 0..field_ids_size {
            let base = field_ids_off + i * 8;
            let c_idx = r_u16(raw, base).unwrap_or(0);
            let t_idx = r_u16(raw, base + 2).unwrap_or(0);
            let n_idx = r_u32(raw, base + 4).unwrap_or(0);
            fields.push((c_idx, t_idx, n_idx));
        }

        // 5. Read Methods (class_idx, proto_idx, name_idx)
        let mut methods: Vec<(u16, u16, u32)> = Vec::with_capacity(method_ids_size.min(300_000));
        for i in 0..method_ids_size {
            let base = method_ids_off + i * 8;
            let c_idx = r_u16(raw, base).unwrap_or(0);
            let p_idx = r_u16(raw, base + 2).unwrap_or(0);
            let n_idx = r_u32(raw, base + 4).unwrap_or(0);
            methods.push((c_idx, p_idx, n_idx));
        }

        let pkg_root = app_pkg_prefix
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");

        // 6. Walk Class Definitions
        for i in 0..class_defs_size {
            let base = class_defs_off + i * 32;
            let Some(class_idx) = r_u32(raw, base) else {
                break;
            };
            let annotations_off = r_u32(raw, base + 20).unwrap_or(0) as usize;
            let class_data_off = r_u32(raw, base + 24).unwrap_or(0) as usize;

            let raw_desc = types
                .get(class_idx as usize)
                .map(String::as_str)
                .unwrap_or("");
            let cls_name = desc_to_dotted(raw_desc);

            let is_noise_lib = cls_name.starts_with("androidx.")
                || cls_name.starts_with("android.support.")
                || cls_name.starts_with("kotlin.")
                || cls_name.starts_with("kotlinx.")
                || cls_name.starts_with("com.google.android.gms.")
                || cls_name.starts_with("com.google.common.")
                || cls_name.starts_with("com.google.protobuf.")
                || cls_name.starts_with("org.apache.")
                || cls_name.starts_with("io.reactivex.");

            let cls_lower = cls_name.to_ascii_lowercase();
            let is_truck_or_nav_cls = !is_noise_lib
                && (cls_lower.contains("truck")
                    || cls_lower.contains("vehicle")
                    || cls_lower.contains("height")
                    || cls_lower.contains("weight")
                    || cls_lower.contains("toll")
                    || cls_lower.contains("hipass")
                    || cls_lower.contains("minitbt")
                    || cls_lower.contains("cctv")
                    || cls_lower.contains("roadview")
                    || cls_lower.contains("streetview")
                    || cls_lower.contains("panorama")
                    || cls_lower.contains("dispatch")
                    || cls_lower.contains("cargo")
                    || cls_lower.contains("freight")
                    || cls_lower.contains("shipper")
                    || cls_lower.contains("settle"));

            let mut js_bridge_method_ids: BTreeSet<usize> = BTreeSet::new();
            if annotations_off != 0 && !is_noise_lib {
                self.parse_annotations_directory(
                    raw,
                    annotations_off,
                    &cls_name,
                    &strings,
                    &types,
                    &protos,
                    &fields,
                    &methods,
                    &mut js_bridge_method_ids,
                );
            }

            if class_data_off == 0 {
                continue;
            }

            let Some((sf_sz, mut p)) = r_uleb128(raw, class_data_off) else {
                continue;
            };
            let Some((if_sz, p2)) = r_uleb128(raw, p) else {
                continue;
            };
            p = p2;
            let Some((dm_sz, p3)) = r_uleb128(raw, p) else {
                continue;
            };
            p = p3;
            let Some((vm_sz, p4)) = r_uleb128(raw, p) else {
                continue;
            };
            p = p4;

            for _ in 0..(sf_sz + if_sz) {
                let Some((_f_diff, np)) = r_uleb128(raw, p) else {
                    break;
                };
                let Some((_acc, np2)) = r_uleb128(raw, np) else {
                    break;
                };
                p = np2;
            }

            let mut cls_member_names = Vec::new();
            let is_app_domain = (!pkg_root.is_empty() && cls_name.starts_with(&pkg_root))
                || cls_name.starts_with("com.naver.map")
                || cls_name.starts_with("com.nhn.android")
                || cls_name.starts_with("com.kakao.map")
                || cls_name.starts_with("net.daum.android")
                || cls_name.starts_with("kr.mappers")
                || cls_name.starts_with("com.call24")
                || cls_name.starts_with("com.gu.fns")
                || cls_name.starts_with("ktc.cargo")
                || cls_name.starts_with("kic.owner")
                || cls_name.starts_with("com.sanggong")
                || cls_name.starts_with("com.pairip");

            for group_sz in [dm_sz, vm_sz] {
                let mut m_idx = 0usize;
                for _ in 0..group_sz {
                    let Some((m_diff, np)) = r_uleb128(raw, p) else {
                        break;
                    };
                    let Some((acc, np2)) = r_uleb128(raw, np) else {
                        break;
                    };
                    let Some((code_off, np3)) = r_uleb128(raw, np2) else {
                        break;
                    };
                    p = np3;
                    m_idx = m_idx.saturating_add(m_diff as usize);

                    let Some(&(_mc, mp_idx, mn_idx)) = methods.get(m_idx) else {
                        continue;
                    };
                    let m_name = strings.get(mn_idx as usize).cloned().unwrap_or_default();
                    let m_sig = protos.get(mp_idx as usize).cloned().unwrap_or_default();

                    if is_truck_or_nav_cls && cls_member_names.len() < 24 {
                        cls_member_names.push(format!("{}{}", m_name, m_sig));
                    }

                    if (acc & ACC_NATIVE) != 0
                        && !is_noise_lib
                        && self.native_declarations.len() < 600
                    {
                        self.native_declarations.push(DexNativeMethodDecl {
                            class_name: cls_name.clone(),
                            method_name: m_name.clone(),
                            jni_descriptor: m_sig.clone(),
                            is_static: (acc & ACC_STATIC) != 0,
                        });
                    }

                    if code_off != 0
                        && (is_app_domain
                            || is_truck_or_nav_cls
                            || js_bridge_method_ids.contains(&m_idx))
                    {
                        self.inspect_method_bytecode(
                            raw,
                            code_off as usize,
                            &cls_name,
                            &m_name,
                            &m_sig,
                            acc,
                            js_bridge_method_ids.contains(&m_idx),
                            is_truck_or_nav_cls,
                            &strings,
                            &types,
                            &fields,
                            &methods,
                        );
                    }
                }
            }

            if is_truck_or_nav_cls && self.truck_and_routing_classes.len() < 220 {
                self.truck_and_routing_classes
                    .push((cls_name, cls_member_names));
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_annotations_directory(
        &mut self,
        raw: &[u8],
        dir_off: usize,
        cls_name: &str,
        strings: &[String],
        types: &[String],
        protos: &[String],
        fields: &[(u16, u16, u32)],
        methods: &[(u16, u16, u32)],
        js_bridge_method_ids: &mut BTreeSet<usize>,
    ) {
        let Some(cls_ann_off) = r_u32(raw, dir_off) else {
            return;
        };
        let fields_sz = r_u32(raw, dir_off + 4).unwrap_or(0) as usize;
        let methods_sz = r_u32(raw, dir_off + 8).unwrap_or(0) as usize;

        if cls_ann_off != 0 {
            for (ann_type, kv) in read_annotation_set(raw, cls_ann_off as usize, strings, types) {
                if ann_type.ends_with(".Entity") && self.room_entities_and_queries.len() < 200 {
                    let tbl = kv
                        .get("tableName")
                        .and_then(|v| v.first())
                        .cloned()
                        .unwrap_or_else(|| cls_name.to_string());
                    self.room_entities_and_queries.push(DexRoomQuery {
                        dao_class: cls_name.to_string(),
                        method_name: "<class>".into(),
                        annotation_kind: "@Entity".into(),
                        sql_or_table: tbl,
                    });
                }
            }
        }

        let mut cur = dir_off + 16;
        for _ in 0..fields_sz.min(256) {
            let (Some(f_idx), Some(a_off)) = (r_u32(raw, cur), r_u32(raw, cur + 4)) else {
                break;
            };
            cur += 8;
            if self.dto_serialized_fields.len() < 350 {
                let f_name = fields
                    .get(f_idx as usize)
                    .and_then(|&(_, _, ni)| strings.get(ni as usize))
                    .cloned()
                    .unwrap_or_default();
                for (ann_type, kv) in read_annotation_set(raw, a_off as usize, strings, types) {
                    if ann_type.ends_with("SerializedName")
                        || ann_type.ends_with("JsonProperty")
                        || ann_type.ends_with("Json")
                    {
                        if let Some(val) = kv
                            .get("value")
                            .or_else(|| kv.get("name"))
                            .and_then(|v| v.first())
                        {
                            self.dto_serialized_fields.push((
                                cls_name.to_string(),
                                f_name.clone(),
                                val.clone(),
                            ));
                        }
                    }
                }
            }
        }

        for _ in 0..methods_sz.min(512) {
            let (Some(m_idx), Some(a_off)) = (r_u32(raw, cur), r_u32(raw, cur + 4)) else {
                break;
            };
            cur += 8;
            let (m_name, m_sig) = methods
                .get(m_idx as usize)
                .map(|&(_, pi, ni)| {
                    (
                        strings.get(ni as usize).cloned().unwrap_or_default(),
                        protos.get(pi as usize).cloned().unwrap_or_default(),
                    )
                })
                .unwrap_or_default();

            for (ann_type, kv) in read_annotation_set(raw, a_off as usize, strings, types) {
                let short = ann_type.rsplit('.').next().unwrap_or(&ann_type);
                match short {
                    "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "HTTP" => {
                        if (ann_type.contains("retrofit") || ann_type.contains("http"))
                            && self.retrofit_endpoints.len() < 500
                        {
                            let path = kv
                                .get("value")
                                .or_else(|| kv.get("path"))
                                .and_then(|v| v.first())
                                .cloned()
                                .unwrap_or_default();
                            self.retrofit_endpoints.push(DexHttpEndpoint {
                                http_method: short.to_string(),
                                path,
                                host_class: cls_name.to_string(),
                                java_method: m_name.clone(),
                                signature: m_sig.clone(),
                            });
                        }
                    }
                    "Query" => {
                        if self.room_entities_and_queries.len() < 200 {
                            if let Some(sql) = kv.get("value").and_then(|v| v.first()) {
                                self.room_entities_and_queries.push(DexRoomQuery {
                                    dao_class: cls_name.to_string(),
                                    method_name: m_name.clone(),
                                    annotation_kind: "@Query".into(),
                                    sql_or_table: sql.clone(),
                                });
                            }
                        }
                    }
                    "JavascriptInterface" => {
                        js_bridge_method_ids.insert(m_idx as usize);
                    }
                    _ => {}
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn inspect_method_bytecode(
        &mut self,
        raw: &[u8],
        code_off: usize,
        cls_name: &str,
        m_name: &str,
        m_sig: &str,
        acc: u32,
        is_js_bridge: bool,
        is_truck_cls: bool,
        strings: &[String],
        types: &[String],
        fields: &[(u16, u16, u32)],
        methods: &[(u16, u16, u32)],
    ) {
        let registers_size = r_u16(raw, code_off).unwrap_or(0);
        let ins_size = r_u16(raw, code_off + 2).unwrap_or(0);
        let Some(insns_size_u32) = r_u32(raw, code_off + 12) else {
            return;
        };
        let insns_units = (insns_size_u32 as usize).min(4096);
        let insn_base = code_off + 16;
        if insn_base + insns_units * 2 > raw.len() {
            return;
        }

        // Quick pre-scan for branch targets so CFG block counts & Java labels are exact!
        let mut branch_leaders: BTreeSet<usize> = BTreeSet::new();
        branch_leaders.insert(0);
        let mut branch_count = 0usize;
        let mut scan_pc = 0usize;
        while scan_pc < insns_units {
            let Some(u0) = r_u16(raw, insn_base + scan_pc * 2) else {
                break;
            };
            let op = (u0 & 0xff) as u8;
            let step = dalvik_opcode_width(op, raw, insn_base + scan_pc * 2).max(1);
            match op {
                0x28 => {
                    branch_count += 1;
                    let rel = ((u0 >> 8) as i8) as isize;
                    let tgt = (scan_pc as isize + rel).max(0) as usize;
                    branch_leaders.insert(tgt);
                    branch_leaders.insert(scan_pc + step);
                }
                0x29 => {
                    branch_count += 1;
                    let rel = r_u16(raw, insn_base + (scan_pc + 1) * 2).unwrap_or(0) as i16 as isize;
                    let tgt = (scan_pc as isize + rel).max(0) as usize;
                    branch_leaders.insert(tgt);
                    branch_leaders.insert(scan_pc + step);
                }
                0x32..=0x3d => {
                    branch_count += 1;
                    let rel = r_u16(raw, insn_base + (scan_pc + 1) * 2).unwrap_or(0) as i16 as isize;
                    let tgt = (scan_pc as isize + rel).max(0) as usize;
                    branch_leaders.insert(tgt);
                    branch_leaders.insert(scan_pc + step);
                }
                0x2b | 0x2c => {
                    branch_count += 1;
                    branch_leaders.insert(scan_pc + step);
                }
                _ => {}
            }
            scan_pc += step;
        }
        let cfg_blocks = branch_leaders.len().max(1);

        // Seed Dalvik parameter registers (`this` + `p0..pN`)
        let mut regs: BTreeMap<u16, String> = BTreeMap::new();
        let is_static = (acc & ACC_STATIC) != 0;
        if registers_size >= ins_size && ins_size > 0 {
            let first_in = registers_size - ins_size;
            for idx in 0..ins_size {
                let r = first_in + idx;
                if !is_static && idx == 0 {
                    regs.insert(r, "this".to_string());
                } else {
                    let p_num = if is_static { idx } else { idx - 1 };
                    regs.insert(r, format!("p{}", p_num));
                }
            }
        }

        let reg_val = |m: &BTreeMap<u16, String>, r: u16| -> String {
            m.get(&r).cloned().unwrap_or_else(|| format!("v{}", r))
        };

        let mut const_strs = Vec::new();
        let mut invoked = Vec::new();
        let mut fld_refs = Vec::new();
        let mut preview = Vec::new();
        let mut java_lines: Vec<String> = Vec::new();
        java_lines.push(format!(
            "// Decompiled Dalvik [Units={} BB={} Branches={}] {}.{}{} ",
            insns_units,
            cfg_blocks,
            branch_count,
            short_class_name(cls_name),
            m_name,
            m_sig
        ));

        let mut pending_invoke: Option<String> = None;
        let flush_pending = |pending: &mut Option<String>, out: &mut Vec<String>| {
            if let Some(expr) = pending.take() {
                if out.len() < 42 {
                    out.push(format!("  {};", expr));
                }
            }
        };

        let mut has_crypto = false;
        let mut has_net = false;
        let mut has_db = false;
        let mut has_deeplink = false;
        let mut header_apis: Vec<String> = Vec::new();
        let mut uses_intent = false;

        let mut pc = 0usize;
        while pc < insns_units {
            if pc != 0 && branch_leaders.contains(&pc) {
                flush_pending(&mut pending_invoke, &mut java_lines);
                if java_lines.len() < 42 {
                    java_lines.push(format!("L_{:04x}:", pc));
                }
            }

            let Some(u0) = r_u16(raw, insn_base + pc * 2) else {
                break;
            };
            let op = (u0 & 0xff) as u8;
            let step = dalvik_opcode_width(op, raw, insn_base + pc * 2).max(1);

            if !(0x0a..=0x0c).contains(&op) {
                flush_pending(&mut pending_invoke, &mut java_lines);
            }

            match op {
                0x01 | 0x04 | 0x07 => {
                    // move vA, vB (12x)
                    let dst = ((u0 >> 8) & 0xf) as u16;
                    let src = ((u0 >> 12) & 0xf) as u16;
                    let v = reg_val(&regs, src);
                    regs.insert(dst, v);
                }
                0x02 | 0x05 | 0x08 => {
                    // move/from16 vAA, vBBBB (22x)
                    let dst = u0 >> 8;
                    let src = r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0);
                    let v = reg_val(&regs, src);
                    regs.insert(dst, v);
                }
                0x0a..=0x0c => {
                    // move-result* vAA
                    let dst = u0 >> 8;
                    if let Some(call_expr) = pending_invoke.take() {
                        let var_name = format!("v{}", dst);
                        if java_lines.len() < 42 {
                            java_lines.push(format!("  var {} = {};", var_name, call_expr));
                        }
                        regs.insert(dst, var_name);
                    }
                }
                0x0d => {
                    // move-exception vAA
                    let dst = u0 >> 8;
                    regs.insert(dst, "caughtEx".to_string());
                }
                0x0e => {
                    if java_lines.len() < 42 {
                        java_lines.push("  return;".to_string());
                    }
                }
                0x0f..=0x11 => {
                    let src = u0 >> 8;
                    if java_lines.len() < 42 {
                        java_lines.push(format!("  return {};", reg_val(&regs, src)));
                    }
                }
                0x12 => {
                    // const/4 vA, #+B
                    let dst = ((u0 >> 8) & 0xf) as u16;
                    let imm4 = (((u0 >> 12) as i8) << 4) >> 4;
                    regs.insert(dst, format!("{}", imm4));
                }
                0x13 | 0x16 => {
                    // const/16 or const-wide/16 vAA, #+BBBB
                    let dst = u0 >> 8;
                    let imm16 = r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0) as i16;
                    regs.insert(dst, format!("{}", imm16));
                }
                0x14 | 0x17 => {
                    // const / const-wide/32 vAA, #+BBBBBBBB
                    let dst = u0 >> 8;
                    let lo = r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0) as u32;
                    let hi = r_u16(raw, insn_base + (pc + 2) * 2).unwrap_or(0) as u32;
                    let imm32 = (lo | (hi << 16)) as i32;
                    regs.insert(dst, format!("{}", imm32));
                }
                0x15 => {
                    // const/high16 vAA, #+BBBB0000
                    let dst = u0 >> 8;
                    let hi = (r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0) as u32) << 16;
                    regs.insert(dst, format!("0x{:x}", hi));
                }
                0x1a => {
                    // const-string vAA, string@BBBB
                    let dst = u0 >> 8;
                    if let Some(s_idx) = r_u16(raw, insn_base + (pc + 1) * 2) {
                        if let Some(s) = strings.get(s_idx as usize) {
                            if !s.is_empty() && const_strs.len() < 32 {
                                const_strs.push(s.clone());
                            }
                            regs.insert(dst, format!("{:?}", s));
                            if preview.len() < 28 {
                                preview.push(format!("  {:04x}: const-string v{}, {:?}", pc, dst, s));
                            }
                        }
                    }
                }
                0x1b => {
                    // const-string/jumbo vAA, string@BBBBBBBB
                    let dst = u0 >> 8;
                    if let (Some(lo), Some(hi)) = (
                        r_u16(raw, insn_base + (pc + 1) * 2),
                        r_u16(raw, insn_base + (pc + 2) * 2),
                    ) {
                        let s_idx = (lo as usize) | ((hi as usize) << 16);
                        if let Some(s) = strings.get(s_idx) {
                            if !s.is_empty() && const_strs.len() < 32 {
                                const_strs.push(s.clone());
                            }
                            regs.insert(dst, format!("{:?}", s));
                            if preview.len() < 28 {
                                preview.push(format!(
                                    "  {:04x}: const-string/jumbo v{}, {:?}",
                                    pc, dst, s
                                ));
                            }
                        }
                    }
                }
                0x1c => {
                    // const-class vAA, type@BBBB
                    let dst = u0 >> 8;
                    if let Some(t_idx) = r_u16(raw, insn_base + (pc + 1) * 2) {
                        let t_name = types
                            .get(t_idx as usize)
                            .map(|d| desc_to_dotted(d))
                            .unwrap_or_default();
                        regs.insert(dst, format!("{}.class", short_class_name(&t_name)));
                    }
                }
                0x22 => {
                    // new-instance vAA, type@BBBB
                    let dst = u0 >> 8;
                    if let Some(t_idx) = r_u16(raw, insn_base + (pc + 1) * 2) {
                        let t_name = types
                            .get(t_idx as usize)
                            .map(|d| desc_to_dotted(d))
                            .unwrap_or_default();
                        if t_name == "android.content.Intent"
                            || t_name == "android.content.IntentFilter"
                        {
                            uses_intent = true;
                        }
                        regs.insert(dst, format!("new {}", short_class_name(&t_name)));
                    }
                }
                0x27 => {
                    let src = u0 >> 8;
                    if java_lines.len() < 42 {
                        java_lines.push(format!("  throw {};", reg_val(&regs, src)));
                    }
                }
                0x28 | 0x29 => {
                    let rel = if op == 0x28 {
                        ((u0 >> 8) as i8) as isize
                    } else {
                        r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0) as i16 as isize
                    };
                    let tgt = (pc as isize + rel).max(0) as usize;
                    if java_lines.len() < 42 {
                        java_lines.push(format!("  goto L_{:04x};", tgt));
                    }
                }
                0x32..=0x37 => {
                    // if-test vA, vB, +CCCC
                    let va = ((u0 >> 8) & 0xf) as u16;
                    let vb = ((u0 >> 12) & 0xf) as u16;
                    let rel = r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0) as i16 as isize;
                    let tgt = (pc as isize + rel).max(0) as usize;
                    let cmp_op = match op {
                        0x32 => "==",
                        0x33 => "!=",
                        0x34 => "<",
                        0x35 => ">=",
                        0x36 => ">",
                        _ => "<=",
                    };
                    if java_lines.len() < 42 {
                        java_lines.push(format!(
                            "  if ({} {} {}) goto L_{:04x};",
                            reg_val(&regs, va),
                            cmp_op,
                            reg_val(&regs, vb),
                            tgt
                        ));
                    }
                }
                0x38..=0x3d => {
                    // if-testz vAA, +BBBB
                    let va = u0 >> 8;
                    let rel = r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0) as i16 as isize;
                    let tgt = (pc as isize + rel).max(0) as usize;
                    let cmp_op = match op {
                        0x38 => "== 0",
                        0x39 => "!= 0",
                        0x3a => "< 0",
                        0x3b => ">= 0",
                        0x3c => "> 0",
                        _ => "<= 0",
                    };
                    if java_lines.len() < 42 {
                        java_lines.push(format!(
                            "  if ({} {}) goto L_{:04x};",
                            reg_val(&regs, va),
                            cmp_op,
                            tgt
                        ));
                    }
                }
                0x52..=0x6d => {
                    // iget/iput/sget/sput
                    if let Some(fid) = r_u16(raw, insn_base + (pc + 1) * 2) {
                        if let Some(&(fc, _ft, fn_i)) = fields.get(fid as usize) {
                            let f_cls = types
                                .get(fc as usize)
                                .map(|d| desc_to_dotted(d))
                                .unwrap_or_default();
                            let f_n = strings.get(fn_i as usize).map(String::as_str).unwrap_or("");
                            let full_f = format!("{}.{}", f_cls, f_n);
                            if fld_refs.len() < 20 {
                                fld_refs.push(full_f.clone());
                            }
                            if preview.len() < 28 {
                                let rw = if (0x59..=0x5f).contains(&op) || (0x67..=0x6d).contains(&op)
                                {
                                    "put-field"
                                } else {
                                    "get-field"
                                };
                                preview.push(format!("  {:04x}: {} {}", pc, rw, full_f));
                            }

                            if (0x52..=0x58).contains(&op) {
                                // iget vA, vB, field
                                let va = ((u0 >> 8) & 0xf) as u16;
                                let vb = ((u0 >> 12) & 0xf) as u16;
                                let expr = format!("{}.{}", reg_val(&regs, vb), f_n);
                                regs.insert(va, expr);
                            } else if (0x59..=0x5f).contains(&op) {
                                // iput vA, vB, field
                                let va = ((u0 >> 8) & 0xf) as u16;
                                let vb = ((u0 >> 12) & 0xf) as u16;
                                if java_lines.len() < 42 {
                                    java_lines.push(format!(
                                        "  {}.{} = {};",
                                        reg_val(&regs, vb),
                                        f_n,
                                        reg_val(&regs, va)
                                    ));
                                }
                            } else if (0x60..=0x66).contains(&op) {
                                // sget vAA, field
                                let va = u0 >> 8;
                                regs.insert(va, format!("{}.{}", short_class_name(&f_cls), f_n));
                            } else if (0x67..=0x6d).contains(&op) {
                                // sput vAA, field
                                let va = u0 >> 8;
                                if java_lines.len() < 42 {
                                    java_lines.push(format!(
                                        "  {}.{} = {};",
                                        short_class_name(&f_cls),
                                        f_n,
                                        reg_val(&regs, va)
                                    ));
                                }
                            }
                        }
                    }
                }
                0x6e..=0x72 | 0x74..=0x78 => {
                    // invoke-* (35c) or invoke-*/range (3rc)
                    if let Some(mid) = r_u16(raw, insn_base + (pc + 1) * 2) {
                        if let Some(&(mc, _mp, mn)) = methods.get(mid as usize) {
                            let target_cls = types
                                .get(mc as usize)
                                .map(|d| desc_to_dotted(d))
                                .unwrap_or_default();
                            let target_m =
                                strings.get(mn as usize).map(String::as_str).unwrap_or("");
                            let full_call = format!("{}.{}", target_cls, target_m);

                            // Extract argument registers!
                            let mut arg_regs: Vec<u16> = Vec::new();
                            if (0x6e..=0x72).contains(&op) {
                                let argc = ((u0 >> 12) & 0xf) as usize;
                                let u2 = r_u16(raw, insn_base + (pc + 2) * 2).unwrap_or(0);
                                let nibs = [
                                    u2 & 0xf,
                                    (u2 >> 4) & 0xf,
                                    (u2 >> 8) & 0xf,
                                    (u2 >> 12) & 0xf,
                                    (u0 >> 8) & 0xf,
                                ];
                                for &r in nibs.iter().take(argc.min(5)) {
                                    arg_regs.push(r);
                                }
                            } else {
                                let argc = ((u0 >> 8) & 0xff) as usize;
                                let start_r = r_u16(raw, insn_base + (pc + 2) * 2).unwrap_or(0);
                                for k in 0..argc.min(6) {
                                    arg_regs.push(start_r + k as u16);
                                }
                            }

                            let is_static_call = op == 0x71 || op == 0x77;
                            let call_expr = if is_static_call {
                                let args_s: Vec<String> =
                                    arg_regs.iter().map(|&r| reg_val(&regs, r)).collect();
                                format!(
                                    "{}.{}({})",
                                    short_class_name(&target_cls),
                                    target_m,
                                    args_s.join(", ")
                                )
                            } else if let Some((&recv_r, rest)) = arg_regs.split_first() {
                                let args_s: Vec<String> =
                                    rest.iter().map(|&r| reg_val(&regs, r)).collect();
                                if target_m == "<init>" {
                                    let ctor = format!(
                                        "new {}({})",
                                        short_class_name(&target_cls),
                                        args_s.join(", ")
                                    );
                                    regs.insert(recv_r, ctor.clone());
                                    ctor
                                } else {
                                    format!(
                                        "{}.{}({})",
                                        reg_val(&regs, recv_r),
                                        target_m,
                                        args_s.join(", ")
                                    )
                                }
                            } else {
                                format!("{}()", full_call)
                            };

                            if target_m != "<init>" {
                                pending_invoke = Some(call_expr);
                            }

                            if target_cls.starts_with("javax.crypto.")
                                || target_cls.starts_with("java.security.MessageDigest")
                            {
                                has_crypto = true;
                            }
                            if target_cls.starts_with("java.net.Socket")
                                || target_cls.starts_with("okhttp3.")
                                || target_cls.starts_with("java.net.HttpURLConnection")
                                || target_m == "newWebSocket"
                            {
                                has_net = true;
                            }
                            if (target_cls.contains("Request$Builder")
                                || target_cls.contains("Headers$Builder")
                                || target_cls.contains("HttpURLConnection")
                                || target_cls.contains("URLConnection"))
                                && (target_m == "addHeader"
                                    || target_m == "header"
                                    || target_m == "setRequestProperty"
                                    || target_m == "addRequestProperty")
                            {
                                has_net = true;
                                if !header_apis.contains(&full_call) {
                                    header_apis.push(full_call.clone());
                                }
                            }
                            if target_cls.contains("SQLiteDatabase")
                                || target_m == "getSharedPreferences"
                                || target_m == "execSQL"
                                || target_m == "rawQuery"
                            {
                                has_db = true;
                                if target_m == "getSharedPreferences" {
                                    if let Some(last_s) = const_strs.last() {
                                        if self.shared_prefs_names.len() < 120 {
                                            self.shared_prefs_names.insert(last_s.clone());
                                        }
                                    }
                                }
                            }
                            if (target_cls == "android.net.Uri" && target_m == "parse")
                                || (target_cls == "android.content.Intent" && target_m == "getData")
                            {
                                has_deeplink = true;
                            }
                            if target_cls == "android.content.Intent"
                                || target_cls == "android.content.IntentFilter"
                                || target_m == "sendBroadcast"
                                || target_m == "registerReceiver"
                            {
                                uses_intent = true;
                            }

                            if invoked.len() < 32 {
                                invoked.push(full_call.clone());
                            }
                            if preview.len() < 28 {
                                preview.push(format!("  {:04x}: invoke {}()", pc, full_call));
                            }
                        }
                    }
                }
                0x90..=0x97 => {
                    // binop vAA, vBB, vCC
                    let dst = u0 >> 8;
                    let u1 = r_u16(raw, insn_base + (pc + 1) * 2).unwrap_or(0);
                    let lb = u1 & 0xff;
                    let rb = u1 >> 8;
                    let sym = match op {
                        0x90 => "+",
                        0x91 => "-",
                        0x92 => "*",
                        0x93 => "/",
                        0x94 => "%",
                        0x95 => "&",
                        0x96 => "|",
                        _ => "^",
                    };
                    let expr = format!("({} {} {})", reg_val(&regs, lb), sym, reg_val(&regs, rb));
                    regs.insert(dst, expr);
                }
                0xb0..=0xb7 => {
                    // binop/2addr vA, vB
                    let va = ((u0 >> 8) & 0xf) as u16;
                    let vb = ((u0 >> 12) & 0xf) as u16;
                    let sym = match op {
                        0xb0 => "+",
                        0xb1 => "-",
                        0xb2 => "*",
                        0xb3 => "/",
                        0xb4 => "%",
                        0xb5 => "&",
                        0xb6 => "|",
                        _ => "^",
                    };
                    let expr = format!("({} {} {})", reg_val(&regs, va), sym, reg_val(&regs, vb));
                    regs.insert(va, expr);
                }
                _ => {}
            }
            pc += step;
        }
        flush_pending(&mut pending_invoke, &mut java_lines);

        // Harvest custom Intent / Broadcast IPC contract action strings
        if uses_intent && self.intent_ipc_contracts.len() < 150 {
            for s in &const_strs {
                if s.len() >= 10
                    && s.len() <= 140
                    && !s.starts_with("android.intent.action.MAIN")
                    && !s.starts_with("android.intent.action.VIEW")
                    && !s.contains(' ')
                    && (s.contains(".action.")
                        || s.contains(".ACTION_")
                        || s.starts_with("ACTION_")
                        || s.starts_with("com.call24")
                        || s.starts_with("com.gu.fns")
                        || s.starts_with("ktc.")
                        || s.starts_with("kic.")
                        || s.starts_with("kr.mappers")
                        || s.starts_with("com.nhn.")
                        || s.starts_with("com.naver.")
                        || s.starts_with("net.daum.")
                        || s.starts_with("com.kakao."))
                {
                    self.intent_ipc_contracts.insert(s.clone());
                }
            }
        }

        // Harvest OkHttp / HttpURLConnection Request Header Chains
        if !header_apis.is_empty()
            && !const_strs.is_empty()
            && self.okhttp_header_chains.len() < 40
        {
            self.okhttp_header_chains.push(DexHeaderChain {
                class_name: cls_name.to_string(),
                method_name: m_name.to_string(),
                header_apis,
                header_keys_and_values: const_strs.iter().take(16).cloned().collect(),
                decompiled_snippet: java_lines.iter().take(20).cloned().collect(),
            });
        }

        if is_js_bridge && self.js_bridge_methods.len() < 150 {
            self.js_bridge_methods.push(DexJsBridgeMethod {
                bridge_class: cls_name.to_string(),
                method_name: m_name.to_string(),
                signature: m_sig.to_string(),
                strings_in_body: const_strs.clone(),
                calls_in_body: invoked.clone(),
            });
        }

        let _ = acc & ACC_PUBLIC;

        if has_crypto && self.crypto_methods.len() < 40 {
            self.crypto_methods.push(DexDisassembledMethod {
                class_name: cls_name.to_string(),
                method_name: m_name.to_string(),
                signature: m_sig.to_string(),
                category: "crypto",
                insn_units: insns_units,
                cfg_blocks,
                branch_count,
                const_strings: const_strs,
                invoked_methods: invoked,
                accessed_fields: fld_refs,
                bytecode_preview: preview,
                decompiled_java: java_lines,
            });
        } else if has_net && !const_strs.is_empty() && self.network_and_socket_methods.len() < 45 {
            self.network_and_socket_methods.push(DexDisassembledMethod {
                class_name: cls_name.to_string(),
                method_name: m_name.to_string(),
                signature: m_sig.to_string(),
                category: "network",
                insn_units: insns_units,
                cfg_blocks,
                branch_count,
                const_strings: const_strs,
                invoked_methods: invoked,
                accessed_fields: fld_refs,
                bytecode_preview: preview,
                decompiled_java: java_lines,
            });
        } else if has_db && !const_strs.is_empty() && self.database_and_pref_methods.len() < 40 {
            self.database_and_pref_methods.push(DexDisassembledMethod {
                class_name: cls_name.to_string(),
                method_name: m_name.to_string(),
                signature: m_sig.to_string(),
                category: "database",
                insn_units: insns_units,
                cfg_blocks,
                branch_count,
                const_strings: const_strs,
                invoked_methods: invoked,
                accessed_fields: fld_refs,
                bytecode_preview: preview,
                decompiled_java: java_lines,
            });
        } else if (is_truck_cls || has_deeplink)
            && (!const_strs.is_empty() || insns_units >= 12)
            && self.domain_core_methods.len() < 50
        {
            self.domain_core_methods.push(DexDisassembledMethod {
                class_name: cls_name.to_string(),
                method_name: m_name.to_string(),
                signature: m_sig.to_string(),
                category: if is_truck_cls {
                    "truck_routing"
                } else {
                    "deeplink"
                },
                insn_units: insns_units,
                cfg_blocks,
                branch_count,
                const_strings: const_strs,
                invoked_methods: invoked,
                accessed_fields: fld_refs,
                bytecode_preview: preview,
                decompiled_java: java_lines,
            });
        }
    }
}

fn read_annotation_set(
    raw: &[u8],
    set_off: usize,
    strings: &[String],
    types: &[String],
) -> Vec<(String, BTreeMap<String, Vec<String>>)> {
    let mut out = Vec::new();
    let Some(sz) = r_u32(raw, set_off) else {
        return out;
    };
    for i in 0..(sz as usize).min(32) {
        let Some(item_off) = r_u32(raw, set_off + 4 + i * 4) else {
            break;
        };
        let pos = item_off as usize + 1;
        let Some((type_idx, mut p)) = r_uleb128(raw, pos) else {
            continue;
        };
        let Some((elem_cnt, p2)) = r_uleb128(raw, p) else {
            continue;
        };
        p = p2;
        let ann_type = types
            .get(type_idx as usize)
            .map(|d| desc_to_dotted(d))
            .unwrap_or_default();
        let mut kv = BTreeMap::new();
        for _ in 0..(elem_cnt as usize).min(32) {
            let Some((name_idx, vp)) = r_uleb128(raw, p) else {
                break;
            };
            let k_name = strings.get(name_idx as usize).cloned().unwrap_or_default();
            if let Some((vals, next_p)) = read_encoded_string_or_array(raw, vp, strings) {
                if !vals.is_empty() {
                    kv.insert(k_name, vals);
                }
                p = next_p;
            } else {
                break;
            }
        }
        out.push((ann_type, kv));
    }
    out
}

/// Exact Dalvik bytecode instruction width in 16-bit code units (Android Dalvik bytecode table).
fn dalvik_opcode_width(op: u8, raw: &[u8], byte_off: usize) -> usize {
    if op == 0x00 {
        let u0 = r_u16(raw, byte_off).unwrap_or(0);
        match u0 {
            0x0100 => {
                let sz = r_u16(raw, byte_off + 2).unwrap_or(0) as usize;
                return 4 + sz * 2;
            }
            0x0200 => {
                let sz = r_u16(raw, byte_off + 2).unwrap_or(0) as usize;
                return 2 + sz * 4;
            }
            0x0300 => {
                let elem_w = r_u16(raw, byte_off + 2).unwrap_or(0) as usize;
                let sz = r_u32(raw, byte_off + 4).unwrap_or(0) as usize;
                let total_bytes = sz.saturating_mul(elem_w);
                return 4 + (total_bytes + 1) / 2;
            }
            _ => return 1,
        }
    }
    match op {
        0x01 | 0x04 | 0x07 | 0x0a..=0x12 | 0x1d..=0x1e | 0x21 | 0x27..=0x28 | 0x3e..=0x43
        | 0x73 | 0x79..=0x8f | 0xb0..=0xcf => 1,
        0x02 | 0x05 | 0x08 | 0x13 | 0x15..=0x16 | 0x19..=0x1a | 0x1c | 0x1f..=0x20
        | 0x22..=0x23 | 0x29 | 0x2d..=0x3d | 0x44..=0x6d | 0x90..=0xaf | 0xd0..=0xe2 => 2,
        0x03 | 0x06 | 0x09 | 0x14 | 0x17 | 0x1b | 0x24..=0x26 | 0x2a..=0x2c | 0x6e..=0x72
        | 0x74..=0x78 => 3,
        0xfa..=0xfd => 4,
        0x18 => 5,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dalvik_opcode_widths_and_descriptors() {
        assert_eq!(
            desc_to_dotted("Lcom/naver/map/TruckRoute;"),
            "com.naver.map.TruckRoute"
        );
        assert_eq!(short_class_name("com.naver.map.TruckRoute"), "TruckRoute");
        assert_eq!(dalvik_opcode_width(0x1a, &[0x1a, 0x00, 0x01, 0x00], 0), 2);
        assert_eq!(
            dalvik_opcode_width(0x6e, &[0x6e, 0x20, 0x05, 0x00, 0x10, 0x00], 0),
            3
        );
        assert_eq!(dalvik_opcode_width(0x18, &[0x18; 10], 0), 5);
    }
}
