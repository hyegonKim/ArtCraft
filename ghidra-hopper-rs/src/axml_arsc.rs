//! Pure-Rust Zero-Dependency Android Binary XML (`AXML`, `0x00080003`) and
//! Android Resource Table (`resources.arsc`, `0x000c0002`) Deep Decoder.
//!
//! Recovers:
//! - Full `AndroidManifest.xml` structure (package, versionCode/Name, min/target/compile SDK,
//!   cleartext traffic & networkSecurityConfig flags, permissions, features, activities,
//!   foreground services + `foregroundServiceType`, receivers, providers + authorities,
//!   intent-filter deep links `scheme://host/path`, and safe redacted `<meta-data>` descriptors)
//! - Auxiliary XML configs (`res/xml/network_security_config.xml`, `file_paths.xml`, etc.)
//! - `resources.arsc` package headers, resource type specs (`layout`, `string`, `xml`, `raw`),
//!   layout screen names, and localized Korean/domain UI strings.

use std::collections::{BTreeMap, BTreeSet};

const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_TABLE_TYPE: u16 = 0x0002;
const RES_XML_TYPE: u16 = 0x0003;
const RES_XML_START_ELEMENT_TYPE: u16 = 0x0102;
const RES_XML_END_ELEMENT_TYPE: u16 = 0x0103;
const RES_XML_RESOURCE_MAP_TYPE: u16 = 0x0180;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_SPEC_TYPE: u16 = 0x0202;

const UTF8_FLAG: u32 = 1 << 8;

#[derive(Debug, Clone, Default)]
pub struct ComponentEntry {
    pub kind: &'static str, // "activity" | "activity-alias" | "service" | "receiver" | "provider"
    pub name: String,
    pub exported: Option<bool>,
    pub permission: Option<String>,
    pub foreground_service_type: Option<String>,
    pub authorities: Option<String>,
    pub process: Option<String>,
    pub actions: Vec<String>,
    pub categories: Vec<String>,
    pub deeplinks: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MetaDataEntry {
    pub name: String,
    pub value_summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct AxmlManifestReport {
    pub package_name: String,
    pub version_code: String,
    pub version_name: String,
    pub min_sdk: String,
    pub target_sdk: String,
    pub compile_sdk: String,
    pub application_class: String,
    pub app_component_factory: String,
    pub uses_cleartext_traffic: Option<bool>,
    pub allow_backup: Option<bool>,
    pub network_security_config: String,
    pub permissions: Vec<String>,
    pub custom_permissions: Vec<String>,
    pub uses_features: Vec<String>,
    pub queries_packages: Vec<String>,
    pub queries_schemes: Vec<String>,
    pub activities: Vec<ComponentEntry>,
    pub services: Vec<ComponentEntry>,
    pub receivers: Vec<ComponentEntry>,
    pub providers: Vec<ComponentEntry>,
    pub all_deeplinks: BTreeSet<String>,
    pub meta_data: Vec<MetaDataEntry>,
    pub xml_preview_lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ArscTableReport {
    pub file_size: usize,
    pub global_string_count: usize,
    pub package_names: Vec<String>,
    pub type_spec_counts: BTreeMap<String, u32>,
    pub layout_names: Vec<String>,
    pub xml_config_names: Vec<String>,
    pub notable_domain_strings: Vec<String>,
}

fn r_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off.checked_add(2)?)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn r_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off.checked_add(4)?)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn r_i32(b: &[u8], off: usize) -> Option<i32> {
    r_u32(b, off).map(|v| v as i32)
}

/// Maps Android framework `android:attr/*` resource IDs (`0x0101xxxx`) to canonical names.
fn android_attr_name_from_resid(res_id: u32) -> Option<&'static str> {
    match res_id {
        0x01010000 => Some("theme"),
        0x01010001 => Some("label"),
        0x01010002 => Some("icon"),
        0x01010003 => Some("name"),
        0x01010006 => Some("permission"),
        0x0101000e => Some("enabled"),
        0x01010010 => Some("exported"),
        0x01010011 => Some("process"),
        0x01010018 => Some("authorities"),
        0x0101001b => Some("grantUriPermissions"),
        0x0101001d => Some("launchMode"),
        0x0101001e => Some("screenOrientation"),
        0x01010024 => Some("value"),
        0x01010025 => Some("resource"),
        0x01010026 => Some("mimeType"),
        0x01010027 => Some("scheme"),
        0x01010028 => Some("host"),
        0x01010029 => Some("port"),
        0x0101002a => Some("path"),
        0x0101002b => Some("pathPrefix"),
        0x0101002c => Some("pathPattern"),
        0x01010202 => Some("targetActivity"),
        0x0101020c => Some("minSdkVersion"),
        0x0101021b => Some("versionCode"),
        0x0101021c => Some("versionName"),
        0x01010270 => Some("targetSdkVersion"),
        0x01010280 => Some("allowBackup"),
        0x01010281 => Some("required"),
        0x010104ec => Some("usesCleartextTraffic"),
        0x01010527 => Some("networkSecurityConfig"),
        0x01010572 => Some("compileSdkVersion"),
        0x01010573 => Some("compileSdkVersionCodename"),
        0x0101057a => Some("appComponentFactory"),
        0x01010599 => Some("foregroundServiceType"),
        _ => None,
    }
}

fn decode_fgs_type(mask: u32) -> String {
    if mask == 0 {
        return "none".to_string();
    }
    let flags = [
        (0x0000_0001, "dataSync"),
        (0x0000_0002, "mediaPlayback"),
        (0x0000_0004, "phoneCall"),
        (0x0000_0008, "location"),
        (0x0000_0010, "connectedDevice"),
        (0x0000_0020, "mediaProjection"),
        (0x0000_0040, "camera"),
        (0x0000_0080, "microphone"),
        (0x0000_0100, "health"),
        (0x0000_0200, "remoteMessaging"),
        (0x0000_0400, "systemExempted"),
        (0x0000_0800, "shortService"),
        (0x4000_0000, "specialUse"),
    ];
    let mut parts = Vec::new();
    for (bit, label) in flags {
        if (mask & bit) != 0 {
            parts.push(label);
        }
    }
    if parts.is_empty() {
        format!("0x{:x}", mask)
    } else {
        parts.join("|")
    }
}

/// Parses a `RES_STRING_POOL_TYPE` (`0x0001`) chunk starting at `chunk_off`.
pub fn parse_res_string_pool(buf: &[u8], chunk_off: usize) -> Vec<String> {
    let mut out = Vec::new();
    let Some(header_size) = r_u16(buf, chunk_off + 2).map(|v| v as usize) else {
        return out;
    };
    let Some(chunk_size) = r_u32(buf, chunk_off + 4).map(|v| v as usize) else {
        return out;
    };
    let Some(string_count) = r_u32(buf, chunk_off + 8).map(|v| v as usize) else {
        return out;
    };
    let flags = r_u32(buf, chunk_off + 16).unwrap_or(0);
    let Some(strings_start) = r_u32(buf, chunk_off + 20).map(|v| v as usize) else {
        return out;
    };
    let is_utf8 = (flags & UTF8_FLAG) != 0;
    let offsets_base = chunk_off + header_size;
    let data_base = chunk_off + strings_start;
    let chunk_end = (chunk_off + chunk_size).min(buf.len());

    out.reserve(string_count.min(200_000));
    for i in 0..string_count.min(500_000) {
        let Some(rel_off) = r_u32(buf, offsets_base + i * 4).map(|v| v as usize) else {
            break;
        };
        let mut p = data_base + rel_off;
        if p >= chunk_end {
            out.push(String::new());
            continue;
        }
        if is_utf8 {
            // UTF-16 length (1 or 2 bytes)
            let b0 = buf[p];
            p += 1;
            if (b0 & 0x80) != 0 {
                p += 1;
            }
            if p >= chunk_end {
                out.push(String::new());
                continue;
            }
            // UTF-8 byte length (1 or 2 bytes)
            let l0 = buf[p] as usize;
            p += 1;
            let byte_len = if (l0 & 0x80) != 0 {
                if p >= chunk_end {
                    out.push(String::new());
                    continue;
                }
                let l1 = buf[p] as usize;
                p += 1;
                ((l0 & 0x7f) << 8) | l1
            } else {
                l0
            };
            let end = (p + byte_len).min(chunk_end);
            if p <= end {
                out.push(String::from_utf8_lossy(&buf[p..end]).into_owned());
            } else {
                out.push(String::new());
            }
        } else {
            // UTF-16LE length (1 or 2 u16 words)
            let Some(u0) = r_u16(buf, p) else {
                out.push(String::new());
                continue;
            };
            p += 2;
            let char_len = if (u0 & 0x8000) != 0 {
                let u1 = r_u16(buf, p).unwrap_or(0) as usize;
                p += 2;
                (((u0 as usize) & 0x7fff) << 16) | u1
            } else {
                u0 as usize
            };
            let max_chars = char_len.min((chunk_end.saturating_sub(p)) / 2);
            let mut u16s = Vec::with_capacity(max_chars.min(4096));
            for c in 0..max_chars.min(8192) {
                if let Some(ch) = r_u16(buf, p + c * 2) {
                    u16s.push(ch);
                }
            }
            out.push(String::from_utf16_lossy(&u16s));
        }
    }
    out
}

fn redact_if_sensitive_meta(meta_name: &str, val: &str) -> String {
    let low = meta_name.to_ascii_lowercase();
    let looks_sensitive_name = low.contains("key")
        || low.contains("secret")
        || low.contains("token")
        || low.contains("client_id")
        || low.contains("app_id")
        || low.contains("apikey");
    if looks_sensitive_name
        && val.len() >= 12
        && !val.starts_with("@0x")
        && !val.starts_with("true")
        && !val.starts_with("false")
    {
        return format!("<REDACTED_CONFIG_VALUE_LEN_{}>", val.len());
    }
    val.to_string()
}

impl AxmlManifestReport {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 8 {
            return Err("AXML buffer too short".to_string());
        }
        let magic_type = r_u16(bytes, 0).unwrap_or(0);
        if magic_type != RES_XML_TYPE {
            return Err(format!("Invalid AXML chunk type: 0x{:04x}", magic_type));
        }
        let header_size = r_u16(bytes, 2).unwrap_or(8) as usize;
        let total_size = r_u32(bytes, 4).unwrap_or(bytes.len() as u32) as usize;
        let limit = total_size.min(bytes.len());

        let mut strings: Vec<String> = Vec::new();
        let mut res_ids: Vec<u32> = Vec::new();
        let mut report = AxmlManifestReport::default();

        let mut tag_stack: Vec<String> = Vec::new();
        let mut current_comp: Option<ComponentEntry> = None;
        let mut in_intent_filter = false;
        let mut in_queries = false;

        let mut pos = header_size;
        while pos + 8 <= limit {
            let c_type = r_u16(bytes, pos).unwrap_or(0);
            let c_hdr = r_u16(bytes, pos + 2).unwrap_or(8) as usize;
            let c_size = r_u32(bytes, pos + 4).unwrap_or(0) as usize;
            if c_size < 8 || pos + c_size > limit {
                break;
            }

            match c_type {
                RES_STRING_POOL_TYPE => {
                    strings = parse_res_string_pool(bytes, pos);
                }
                RES_XML_RESOURCE_MAP_TYPE => {
                    let count = (c_size.saturating_sub(c_hdr)) / 4;
                    for i in 0..count {
                        if let Some(id) = r_u32(bytes, pos + c_hdr + i * 4) {
                            res_ids.push(id);
                        }
                    }
                }
                RES_XML_START_ELEMENT_TYPE => {
                    let elem_name_idx = r_i32(bytes, pos + 20).unwrap_or(-1);
                    let attr_start = r_u16(bytes, pos + 24).unwrap_or(20) as usize;
                    let attr_size = r_u16(bytes, pos + 26).unwrap_or(20) as usize;
                    let attr_count = r_u16(bytes, pos + 28).unwrap_or(0) as usize;

                    let tag = if elem_name_idx >= 0 {
                        strings
                            .get(elem_name_idx as usize)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    let mut attrs: BTreeMap<String, (u8, u32, String)> = BTreeMap::new();
                    let attr_base = pos + 16 + attr_start;
                    if attr_size >= 20 {
                        for a in 0..attr_count.min(128) {
                            let ao = attr_base + a * attr_size;
                            if ao + 20 > pos + c_size {
                                break;
                            }
                            let a_name_idx = r_i32(bytes, ao + 4).unwrap_or(-1);
                            let a_raw_val = r_i32(bytes, ao + 8).unwrap_or(-1);
                            let a_type = bytes.get(ao + 15).copied().unwrap_or(0);
                            let a_data = r_u32(bytes, ao + 16).unwrap_or(0);

                            let mut a_name = if a_name_idx >= 0 {
                                strings
                                    .get(a_name_idx as usize)
                                    .cloned()
                                    .unwrap_or_default()
                            } else {
                                String::new()
                            };
                            if (a_name.is_empty() || a_name == "?") && a_name_idx >= 0 {
                                if let Some(&rid) = res_ids.get(a_name_idx as usize) {
                                    if let Some(mapped) = android_attr_name_from_resid(rid) {
                                        a_name = mapped.to_string();
                                    } else {
                                        a_name = format!("attr_0x{:08x}", rid);
                                    }
                                }
                            } else if a_name_idx >= 0 {
                                if let Some(&rid) = res_ids.get(a_name_idx as usize) {
                                    if a_name.is_empty() {
                                        if let Some(mapped) = android_attr_name_from_resid(rid) {
                                            a_name = mapped.to_string();
                                        }
                                    }
                                }
                            }

                            let formatted = match a_type {
                                0x03 => {
                                    if a_raw_val >= 0 {
                                        strings
                                            .get(a_raw_val as usize)
                                            .cloned()
                                            .unwrap_or_default()
                                    } else {
                                        strings
                                            .get(a_data as usize)
                                            .cloned()
                                            .unwrap_or_default()
                                    }
                                }
                                0x10 => (a_data as i32).to_string(),
                                0x11 => format!("0x{:x}", a_data),
                                0x12 => {
                                    if a_data != 0 {
                                        "true".to_string()
                                    } else {
                                        "false".to_string()
                                    }
                                }
                                0x01 => format!("@0x{:08x}", a_data),
                                _ => {
                                    if a_raw_val >= 0 {
                                        strings
                                            .get(a_raw_val as usize)
                                            .cloned()
                                            .unwrap_or_else(|| format!("0x{:x}", a_data))
                                    } else {
                                        format!("0x{:x}", a_data)
                                    }
                                }
                            };
                            attrs.insert(a_name, (a_type, a_data, formatted));
                        }
                    }

                    // Preview XML line (cap at 180 lines)
                    if report.xml_preview_lines.len() < 180 {
                        let indent = "  ".repeat(tag_stack.len().min(6));
                        let mut attr_strs = Vec::new();
                        for (k, (_, _, v)) in &attrs {
                            let safe_v = if tag == "meta-data" && k == "value" {
                                let mname = attrs
                                    .get("name")
                                    .map(|x| x.2.as_str())
                                    .unwrap_or("");
                                redact_if_sensitive_meta(mname, v)
                            } else {
                                v.clone()
                            };
                            attr_strs.push(format!("{}=\"{}\"", k, safe_v));
                        }
                        if attr_strs.is_empty() {
                            report.xml_preview_lines.push(format!("{}<{}>", indent, tag));
                        } else {
                            report.xml_preview_lines.push(format!(
                                "{}<{} {}>",
                                indent,
                                tag,
                                attr_strs.join(" ")
                            ));
                        }
                    }

                    let get_s = |k: &str| -> String {
                        attrs.get(k).map(|x| x.2.clone()).unwrap_or_default()
                    };
                    let get_b = |k: &str| -> Option<bool> {
                        attrs.get(k).map(|x| x.1 != 0 || x.2 == "true")
                    };

                    match tag.as_str() {
                        "manifest" => {
                            report.package_name = get_s("package");
                            report.version_code = get_s("versionCode");
                            report.version_name = get_s("versionName");
                            let csdk = get_s("compileSdkVersion");
                            if !csdk.is_empty() {
                                report.compile_sdk = csdk;
                            }
                        }
                        "uses-sdk" => {
                            report.min_sdk = get_s("minSdkVersion");
                            report.target_sdk = get_s("targetSdkVersion");
                        }
                        "uses-permission" | "uses-permission-sdk-23" => {
                            let p = get_s("name");
                            if !p.is_empty() && !report.permissions.contains(&p) {
                                report.permissions.push(p);
                            }
                        }
                        "permission" => {
                            let p = get_s("name");
                            if !p.is_empty() {
                                report.custom_permissions.push(p);
                            }
                        }
                        "uses-feature" => {
                            let f = get_s("name");
                            if !f.is_empty() {
                                report.uses_features.push(f);
                            }
                        }
                        "queries" => {
                            in_queries = true;
                        }
                        "package" if in_queries => {
                            let n = get_s("name");
                            if !n.is_empty() && !report.queries_packages.contains(&n) {
                                report.queries_packages.push(n);
                            }
                        }
                        "application" => {
                            report.application_class = get_s("name");
                            report.app_component_factory = get_s("appComponentFactory");
                            report.uses_cleartext_traffic = get_b("usesCleartextTraffic");
                            report.allow_backup = get_b("allowBackup");
                            report.network_security_config = get_s("networkSecurityConfig");
                        }
                        "activity" | "activity-alias" | "service" | "receiver" | "provider" => {
                            let kind = match tag.as_str() {
                                "activity" => "activity",
                                "activity-alias" => "activity-alias",
                                "service" => "service",
                                "receiver" => "receiver",
                                _ => "provider",
                            };
                            let mut comp = ComponentEntry {
                                kind,
                                name: get_s("name"),
                                exported: get_b("exported"),
                                permission: {
                                    let p = get_s("permission");
                                    if p.is_empty() { None } else { Some(p) }
                                },
                                foreground_service_type: attrs
                                    .get("foregroundServiceType")
                                    .map(|(_, d, _)| decode_fgs_type(*d)),
                                authorities: {
                                    let a = get_s("authorities");
                                    if a.is_empty() { None } else { Some(a) }
                                },
                                process: {
                                    let pr = get_s("process");
                                    if pr.is_empty() { None } else { Some(pr) }
                                },
                                ..Default::default()
                            };
                            if comp.name.starts_with('.') && !report.package_name.is_empty() {
                                comp.name = format!("{}{}", report.package_name, comp.name);
                            }
                            current_comp = Some(comp);
                        }
                        "intent-filter" => {
                            in_intent_filter = true;
                        }
                        "action" if in_intent_filter => {
                            let act = get_s("name");
                            if let Some(ref mut c) = current_comp {
                                if !act.is_empty() && !c.actions.contains(&act) {
                                    c.actions.push(act);
                                }
                            }
                        }
                        "category" if in_intent_filter => {
                            let cat = get_s("name");
                            if let Some(ref mut c) = current_comp {
                                if !cat.is_empty() && !c.categories.contains(&cat) {
                                    c.categories.push(cat);
                                }
                            }
                        }
                        "data" if in_intent_filter || in_queries => {
                            let scheme = get_s("scheme");
                            let host = get_s("host");
                            let path = {
                                let p = get_s("path");
                                if !p.is_empty() {
                                    p
                                } else {
                                    let pp = get_s("pathPrefix");
                                    if !pp.is_empty() {
                                        format!("{}*", pp)
                                    } else {
                                        get_s("pathPattern")
                                    }
                                }
                            };
                            if in_queries && !scheme.is_empty() && current_comp.is_none() {
                                if !report.queries_schemes.contains(&scheme) {
                                    report.queries_schemes.push(scheme);
                                }
                            } else if !scheme.is_empty() {
                                let uri = if !host.is_empty() {
                                    format!("{}://{}{}", scheme, host, path)
                                } else {
                                    format!("{}://{}", scheme, path)
                                };
                                report.all_deeplinks.insert(uri.clone());
                                if let Some(ref mut c) = current_comp {
                                    if !c.deeplinks.contains(&uri) {
                                        c.deeplinks.push(uri);
                                    }
                                }
                            }
                        }
                        "meta-data" => {
                            let m_name = get_s("name");
                            let raw_val = {
                                let v = get_s("value");
                                if !v.is_empty() {
                                    v
                                } else {
                                    get_s("resource")
                                }
                            };
                            if !m_name.is_empty() && report.meta_data.len() < 120 {
                                report.meta_data.push(MetaDataEntry {
                                    value_summary: redact_if_sensitive_meta(&m_name, &raw_val),
                                    name: m_name,
                                });
                            }
                        }
                        _ => {}
                    }

                    tag_stack.push(tag);
                }
                RES_XML_END_ELEMENT_TYPE => {
                    let ended = tag_stack.pop().unwrap_or_default();
                    match ended.as_str() {
                        "intent-filter" => in_intent_filter = false,
                        "queries" => in_queries = false,
                        "activity" | "activity-alias" => {
                            if let Some(c) = current_comp.take() {
                                report.activities.push(c);
                            }
                        }
                        "service" => {
                            if let Some(c) = current_comp.take() {
                                report.services.push(c);
                            }
                        }
                        "receiver" => {
                            if let Some(c) = current_comp.take() {
                                report.receivers.push(c);
                            }
                        }
                        "provider" => {
                            if let Some(c) = current_comp.take() {
                                report.providers.push(c);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            pos += c_size;
        }

        Ok(report)
    }
}

impl ArscTableReport {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 12 {
            return Err("ARSC buffer too short".to_string());
        }
        let magic = r_u16(bytes, 0).unwrap_or(0);
        if magic != RES_TABLE_TYPE {
            return Err(format!("Invalid ARSC chunk type: 0x{:04x}", magic));
        }
        let header_size = r_u16(bytes, 2).unwrap_or(12) as usize;
        let total_size = r_u32(bytes, 4).unwrap_or(bytes.len() as u32) as usize;
        let limit = total_size.min(bytes.len());

        let mut report = ArscTableReport {
            file_size: bytes.len(),
            ..Default::default()
        };

        let mut pos = header_size;
        while pos + 8 <= limit {
            let c_type = r_u16(bytes, pos).unwrap_or(0);
            let c_hdr = r_u16(bytes, pos + 2).unwrap_or(8) as usize;
            let c_size = r_u32(bytes, pos + 4).unwrap_or(0) as usize;
            if c_size < 8 || pos + c_size > limit {
                break;
            }

            match c_type {
                RES_STRING_POOL_TYPE => {
                    let pool = parse_res_string_pool(bytes, pos);
                    report.global_string_count = pool.len();
                    let mut layouts = BTreeSet::new();
                    let mut xmls = BTreeSet::new();
                    let mut domain_strs = BTreeSet::new();
                    for s in &pool {
                        if s.starts_with("res/layout/") && s.ends_with(".xml") && layouts.len() < 140
                        {
                            layouts.insert(s.clone());
                        } else if s.starts_with("res/xml/") && xmls.len() < 60 {
                            xmls.insert(s.clone());
                        } else if domain_strs.len() < 140
                            && s.len() >= 4
                            && s.len() <= 140
                            && (s.contains("배차")
                                || s.contains("화물")
                                || s.contains("차주")
                                || s.contains("화주")
                                || s.contains("운임")
                                || s.contains("세금계산서")
                                || s.contains("출동")
                                || s.contains("정비")
                                || s.contains("총중량")
                                || s.contains("높이")
                                || s.contains("차고지")
                                || s.contains("://"))
                        {
                            domain_strs.insert(s.clone());
                        }
                    }
                    report.layout_names = layouts.into_iter().collect();
                    report.xml_config_names = xmls.into_iter().collect();
                    report.notable_domain_strings = domain_strs.into_iter().collect();
                }
                RES_TABLE_PACKAGE_TYPE => {
                    // Package name is 128 UTF-16LE code units at pos + 12
                    if pos + 268 <= pos + c_size {
                        let mut u16s = Vec::with_capacity(128);
                        for i in 0..128 {
                            if let Some(ch) = r_u16(bytes, pos + 12 + i * 2) {
                                if ch == 0 {
                                    break;
                                }
                                u16s.push(ch);
                            }
                        }
                        let pkg_name = String::from_utf16_lossy(&u16s);
                        if !pkg_name.is_empty() {
                            report.package_names.push(pkg_name);
                        }
                    }
                    let type_strings_off = r_u32(bytes, pos + 268).unwrap_or(0) as usize;
                    let mut type_names: Vec<String> = Vec::new();
                    if type_strings_off > 0 && pos + type_strings_off + 8 <= pos + c_size {
                        type_names = parse_res_string_pool(bytes, pos + type_strings_off);
                    }
                    // Walk subchunks inside package
                    let mut p2 = pos + c_hdr;
                    let pkg_end = pos + c_size;
                    while p2 + 8 <= pkg_end {
                        let st = r_u16(bytes, p2).unwrap_or(0);
                        let sz = r_u32(bytes, p2 + 4).unwrap_or(0) as usize;
                        if sz < 8 || p2 + sz > pkg_end {
                            break;
                        }
                        if st == RES_TABLE_TYPE_SPEC_TYPE && sz >= 16 {
                            let type_id = bytes.get(p2 + 8).copied().unwrap_or(0) as usize;
                            let entry_count = r_u32(bytes, p2 + 12).unwrap_or(0);
                            let tname = if type_id > 0 {
                                type_names
                                    .get(type_id - 1)
                                    .cloned()
                                    .unwrap_or_else(|| format!("type_{}", type_id))
                            } else {
                                "type_0".to_string()
                            };
                            *report.type_spec_counts.entry(tname).or_insert(0) += entry_count;
                        }
                        p2 += sz;
                    }
                }
                _ => {}
            }

            pos += c_size;
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attr_mapping_and_redaction() {
        assert_eq!(android_attr_name_from_resid(0x01010003), Some("name"));
        assert_eq!(
            android_attr_name_from_resid(0x01010599),
            Some("foregroundServiceType")
        );
        assert_eq!(decode_fgs_type(0x09), "dataSync|location");
        assert_eq!(
            redact_if_sensitive_meta("com.kakao.sdk.AppKey", "abcdef0123456789abcdef0123456789"),
            "<REDACTED_CONFIG_VALUE_LEN_32>"
        );
        assert_eq!(
            redact_if_sensitive_meta("androidx.car.app.minCarApiLevel", "1"),
            "1"
        );
    }
}
