//! Zero-Dependency Pure-Rust JSON Serializer v0.5.0 for `Elf64DeepReport`, `MultiDexDeepReport`,
//! `AxmlManifestReport`, and `ArscTableReport`.

use crate::android_dex::{DexDisassembledMethod, MultiDexDeepReport};
use crate::axml_arsc::{ArscTableReport, AxmlManifestReport, ComponentEntry};
use crate::elf_arm64::{Elf64DeepReport, JniExportDecompilation};
use crate::pcode_sleigh::CfgFunctionMetrics;

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_str_array<'a, I>(iter: I) -> String
where
    I: IntoIterator<Item = &'a String>,
{
    let items: Vec<String> = iter.into_iter().map(|s| json_escape(s)).collect();
    format!("[{}]", items.join(", "))
}

fn serialize_cfg_metrics(m: &CfgFunctionMetrics) -> String {
    let ratio = if m.decoded_ratio_percent.is_finite() {
        m.decoded_ratio_percent
    } else {
        0.0
    };
    let pp_hex: Vec<String> = m
        .dart_pp_offsets
        .iter()
        .map(|o| format!("\"0x{:x}\"", o))
        .collect();
    format!(
        "{{ \"insn_count\": {}, \"decoded_ratio_percent\": {:.1}, \"basic_blocks\": {}, \"cfg_edges\": {}, \"cyclomatic_complexity\": {}, \"loop_backedges\": {}, \"jni_env_calls\": {}, \"dart_pp_offsets\": [{}] }}",
        m.insn_count,
        ratio,
        m.basic_blocks,
        m.cfg_edges,
        m.cyclomatic_complexity,
        m.loop_backedges,
        json_str_array(&m.jni_env_calls),
        pp_hex.join(", ")
    )
}

fn serialize_arm64_decomp_list(list: &[JniExportDecompilation]) -> String {
    let mut s = String::from("[\n");
    for (idx, j) in list.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"symbol\": {}, \"demangled\": {}, \"vaddr\": \"0x{:x}\", \"file_offset\": \"0x{:x}\", \"size\": {}, \"string_xrefs\": {}, \"callee_symbols\": {}, \"cfg_metrics\": {}, \"pseudocode\": {} }}{}\n",
            json_escape(&j.symbol),
            json_escape(&j.demangled),
            j.vaddr,
            j.file_offset,
            j.size,
            json_str_array(&j.string_xrefs),
            json_str_array(&j.callee_symbols),
            serialize_cfg_metrics(&j.cfg_metrics),
            json_escape(&j.pseudocode),
            if idx + 1 < list.len() { "," } else { "" }
        ));
    }
    s.push_str("  ]");
    s
}

pub fn elf_report_to_json(so_name: &str, r: &Elf64DeepReport) -> String {
    let mut s = String::with_capacity(96 * 1024);
    s.push_str("{\n");
    s.push_str(&format!("  \"so_name\": {},\n", json_escape(so_name)));
    s.push_str(&format!("  \"arch\": {},\n", json_escape(r.arch)));
    s.push_str(&format!("  \"machine\": {},\n", r.machine));
    s.push_str(&format!("  \"file_size\": {},\n", r.file_size));
    s.push_str(&format!("  \"entry_vaddr\": \"0x{:x}\",\n", r.entry_vaddr));
    let soname_json = match &r.soname {
        Some(v) => json_escape(v),
        None => "null".to_string(),
    };
    s.push_str(&format!("  \"soname\": {},\n", soname_json));
    s.push_str(&format!("  \"dt_needed\": {},\n", json_str_array(&r.dt_needed)));
    s.push_str(&format!(
        "  \"hardening\": {{ \"nx_enabled\": {}, \"relro\": {}, \"stack_canary\": {}, \"fortify_source\": {}, \"stripped_symtab\": {} }},\n",
        r.hardening.nx_enabled,
        json_escape(r.hardening.relro),
        r.hardening.stack_canary,
        r.hardening.fortify_source,
        r.hardening.stripped_symtab
    ));
    s.push_str(&format!("  \"total_dynsym\": {},\n", r.total_dynsym));
    s.push_str(&format!(
        "  \"exported_functions_count\": {},\n",
        r.exported_functions_count
    ));
    s.push_str(&format!(
        "  \"imported_functions_count\": {},\n",
        r.imported_functions.len()
    ));

    // v0.5.0 ARM64 Stripped Binary Call-Graph & Prologue Summary
    let cg = &r.arm64_call_graph;
    s.push_str("  \"arm64_call_graph\": {\n");
    s.push_str(&format!("    \"executable_bytes\": {},\n", cg.executable_bytes));
    s.push_str(&format!(
        "    \"prologue_functions_count\": {},\n",
        cg.prologue_functions_count
    ));
    s.push_str(&format!(
        "    \"paciasp_functions_count\": {},\n",
        cg.paciasp_functions_count
    ));
    s.push_str(&format!(
        "    \"stp_fp_lr_functions_count\": {},\n",
        cg.stp_fp_lr_functions_count
    ));
    s.push_str(&format!(
        "    \"bl_direct_calls_count\": {},\n",
        cg.bl_direct_calls_count
    ));
    s.push_str(&format!(
        "    \"blr_indirect_calls_count\": {},\n",
        cg.blr_indirect_calls_count
    ));
    s.push_str(&format!(
        "    \"dart_pp_loads_count\": {},\n",
        cg.dart_pp_loads_count
    ));
    s.push_str(&format!(
        "    \"dart_thr_loads_count\": {},\n",
        cg.dart_thr_loads_count
    ));
    s.push_str("    \"top_called_functions\": [\n");
    for (idx, (va, sym, fan_in)) in cg.top_called_functions.iter().enumerate() {
        s.push_str(&format!(
            "      {{ \"vaddr\": \"0x{:x}\", \"symbol\": {}, \"fan_in_callers\": {} }}{}\n",
            va,
            json_escape(sym),
            fan_in,
            if idx + 1 < cg.top_called_functions.len() {
                ","
            } else {
                ""
            }
        ));
    }
    s.push_str("    ]\n  },\n");

    // Sections
    s.push_str("  \"sections\": [\n");
    for (idx, sec) in r.sections.iter().enumerate() {
        let ent = if sec.entropy.is_finite() {
            sec.entropy
        } else {
            0.0
        };
        s.push_str(&format!(
            "    {{ \"name\": {}, \"sh_type\": {}, \"vaddr\": \"0x{:x}\", \"file_offset\": \"0x{:x}\", \"size\": {}, \"entropy\": {:.3} }}{}\n",
            json_escape(&sec.name),
            sec.sh_type,
            sec.vaddr,
            sec.file_offset,
            sec.size,
            ent,
            if idx + 1 < r.sections.len() { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    // JNI Exports with Ghidra P-Code CFG Pseudocode
    s.push_str(&format!(
        "  \"jni_exports\": {},\n",
        serialize_arm64_decomp_list(&r.jni_exports)
    ));

    // v0.5.0 Top Internal / Stripped / Dart AOT Hub Decompilations
    s.push_str(&format!(
        "  \"top_hub_decompilations\": {},\n",
        serialize_arm64_decomp_list(&r.top_hub_decompilations)
    ));

    // RegisterNatives candidates
    s.push_str("  \"jni_registered_candidates\": [\n");
    for (idx, c) in r.jni_registered_candidates.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"method_name\": {}, \"descriptor\": {}, \"fn_vaddr\": \"0x{:x}\", \"table_vaddr\": \"0x{:x}\" }}{}\n",
            json_escape(&c.method_name),
            json_escape(&c.descriptor),
            c.fn_vaddr,
            c.table_vaddr,
            if idx + 1 < r.jni_registered_candidates.len() { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    // Top C++ Classes
    s.push_str("  \"cpp_classes_top\": [\n");
    for (idx, (cls, cnt)) in r.cpp_classes_top.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"class_name\": {}, \"method_count\": {} }}{}\n",
            json_escape(cls),
            cnt,
            if idx + 1 < r.cpp_classes_top.len() { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    // Domain exports sample
    s.push_str("  \"domain_exports_sample\": [\n");
    for (idx, (sym, dem, va)) in r.domain_exports_sample.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"symbol\": {}, \"demangled\": {}, \"vaddr\": \"0x{:x}\" }}{}\n",
            json_escape(sym),
            json_escape(dem),
            va,
            if idx + 1 < r.domain_exports_sample.len() { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    s.push_str(&format!(
        "  \"crypto_and_network_imports\": {},\n",
        json_str_array(&r.crypto_and_network_imports)
    ));
    s.push_str(&format!(
        "  \"url_and_protocol_strings\": {},\n",
        json_str_array(&r.url_and_protocol_strings)
    ));
    s.push_str(&format!(
        "  \"sql_and_schema_strings\": {},\n",
        json_str_array(&r.sql_and_schema_strings)
    ));

    // Flutter AOT (v0.5.0 Binary Snapshot Header + Channels + Classes)
    s.push_str(&format!(
        "  \"flutter_aot\": {{\n    \"is_flutter_aot\": {},\n    \"snapshot_magic_verified\": {},\n    \"vm_snapshot_size\": {},\n    \"isolate_snapshot_size\": {},\n    \"isolate_instructions_size\": {},\n    \"dart_sdk_snapshot_hash\": {},\n    \"snapshot_features\": {},\n    \"method_channels\": {},\n    \"dart_classes\": {},\n    \"dart_symbols\": {},\n    \"package_uris\": {},\n    \"api_urls_and_routes\": {},\n    \"domain_identifiers\": {}\n  }}\n",
        r.flutter_aot.is_flutter_aot,
        r.flutter_aot.snapshot_magic_verified,
        r.flutter_aot.vm_snapshot_size,
        r.flutter_aot.isolate_snapshot_size,
        r.flutter_aot.isolate_instructions_size,
        json_escape(&r.flutter_aot.dart_sdk_snapshot_hash),
        json_str_array(&r.flutter_aot.snapshot_features),
        json_str_array(&r.flutter_aot.method_channels),
        json_str_array(&r.flutter_aot.dart_classes),
        json_str_array(&r.flutter_aot.dart_symbols),
        json_str_array(&r.flutter_aot.package_uris),
        json_str_array(&r.flutter_aot.api_urls_and_routes),
        json_str_array(&r.flutter_aot.domain_identifiers)
    ));

    s.push_str("}\n");
    s
}

fn serialize_disassembled_methods(list: &[DexDisassembledMethod]) -> String {
    let mut s = String::from("[\n");
    for (idx, m) in list.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"class_name\": {}, \"method_name\": {}, \"signature\": {}, \"category\": {}, \"insn_units\": {}, \"cfg_blocks\": {}, \"branch_count\": {}, \"const_strings\": {}, \"invoked_methods\": {}, \"accessed_fields\": {}, \"bytecode_preview\": {}, \"decompiled_java\": {} }}{}\n",
            json_escape(&m.class_name),
            json_escape(&m.method_name),
            json_escape(&m.signature),
            json_escape(m.category),
            m.insn_units,
            m.cfg_blocks,
            m.branch_count,
            json_str_array(&m.const_strings),
            json_str_array(&m.invoked_methods),
            json_str_array(&m.accessed_fields),
            json_str_array(&m.bytecode_preview),
            json_str_array(&m.decompiled_java),
            if idx + 1 < list.len() { "," } else { "" }
        ));
    }
    s.push_str("  ]");
    s
}

pub fn dex_report_to_json(app_pkg: &str, r: &MultiDexDeepReport) -> String {
    let mut s = String::with_capacity(160 * 1024);
    s.push_str("{\n");
    s.push_str(&format!("  \"app_package\": {},\n", json_escape(app_pkg)));
    s.push_str(&format!("  \"dex_files_scanned\": {},\n", r.dex_files_scanned));
    s.push_str(&format!("  \"dex_file_count\": {},\n", r.dex_files_scanned));
    s.push_str(&format!("  \"total_strings\": {},\n", r.total_strings));
    s.push_str(&format!("  \"total_types\": {},\n", r.total_types));
    s.push_str(&format!("  \"total_fields\": {},\n", r.total_fields));
    s.push_str(&format!("  \"total_methods\": {},\n", r.total_methods));
    s.push_str(&format!("  \"total_classes\": {},\n", r.total_classes));

    // Native declarations
    s.push_str("  \"native_declarations\": [\n");
    for (idx, n) in r.native_declarations.iter().take(250).enumerate() {
        let len = r.native_declarations.len().min(250);
        s.push_str(&format!(
            "    {{ \"class_name\": {}, \"method_name\": {}, \"jni_descriptor\": {}, \"is_static\": {} }}{}\n",
            json_escape(&n.class_name),
            json_escape(&n.method_name),
            json_escape(&n.jni_descriptor),
            n.is_static,
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");
    s.push_str(&format!(
        "  \"native_declarations_total\": {},\n",
        r.native_declarations.len()
    ));

    // Retrofit endpoints
    s.push_str("  \"retrofit_endpoints\": [\n");
    for (idx, ep) in r.retrofit_endpoints.iter().take(350).enumerate() {
        let len = r.retrofit_endpoints.len().min(350);
        s.push_str(&format!(
            "    {{ \"http_method\": {}, \"path\": {}, \"host_class\": {}, \"java_method\": {}, \"signature\": {} }}{}\n",
            json_escape(&ep.http_method),
            json_escape(&ep.path),
            json_escape(&ep.host_class),
            json_escape(&ep.java_method),
            json_escape(&ep.signature),
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");
    s.push_str(&format!(
        "  \"retrofit_endpoints_total\": {},\n",
        r.retrofit_endpoints.len()
    ));

    // Room entities and queries
    s.push_str("  \"room_entities_and_queries\": [\n");
    for (idx, rq) in r.room_entities_and_queries.iter().take(250).enumerate() {
        let len = r.room_entities_and_queries.len().min(250);
        s.push_str(&format!(
            "    {{ \"dao_class\": {}, \"method_name\": {}, \"annotation_kind\": {}, \"sql_or_table\": {} }}{}\n",
            json_escape(&rq.dao_class),
            json_escape(&rq.method_name),
            json_escape(&rq.annotation_kind),
            json_escape(&rq.sql_or_table),
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    // JS Bridge methods
    s.push_str("  \"js_bridge_methods\": [\n");
    for (idx, jb) in r.js_bridge_methods.iter().take(200).enumerate() {
        let len = r.js_bridge_methods.len().min(200);
        s.push_str(&format!(
            "    {{ \"bridge_class\": {}, \"method_name\": {}, \"signature\": {}, \"strings_in_body\": {}, \"calls_in_body\": {} }}{}\n",
            json_escape(&jb.bridge_class),
            json_escape(&jb.method_name),
            json_escape(&jb.signature),
            json_str_array(&jb.strings_in_body),
            json_str_array(&jb.calls_in_body),
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    // v0.5.0 OkHttp / HttpURLConnection Request Header Chains
    s.push_str("  \"okhttp_header_chains\": [\n");
    for (idx, hc) in r.okhttp_header_chains.iter().take(40).enumerate() {
        let len = r.okhttp_header_chains.len().min(40);
        s.push_str(&format!(
            "    {{ \"class_name\": {}, \"method_name\": {}, \"header_apis\": {}, \"header_keys_and_values\": {}, \"decompiled_snippet\": {} }}{}\n",
            json_escape(&hc.class_name),
            json_escape(&hc.method_name),
            json_str_array(&hc.header_apis),
            json_str_array(&hc.header_keys_and_values),
            json_str_array(&hc.decompiled_snippet),
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");

    // v0.5.0 Android Intent / Broadcast IPC Contracts
    s.push_str(&format!(
        "  \"intent_ipc_contracts\": {},\n",
        json_str_array(&r.intent_ipc_contracts)
    ));

    // DTO Serialized fields
    s.push_str("  \"dto_serialized_fields\": [\n");
    for (idx, (cls, fld, key)) in r.dto_serialized_fields.iter().take(300).enumerate() {
        let len = r.dto_serialized_fields.len().min(300);
        s.push_str(&format!(
            "    {{ \"class_name\": {}, \"field_name\": {}, \"json_key\": {} }}{}\n",
            json_escape(cls),
            json_escape(fld),
            json_escape(key),
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");
    s.push_str(&format!(
        "  \"dto_serialized_fields_total\": {},\n",
        r.dto_serialized_fields.len()
    ));

    // Truck & routing classes
    s.push_str("  \"truck_and_routing_classes\": [\n");
    for (idx, (cls, mems)) in r.truck_and_routing_classes.iter().take(250).enumerate() {
        let len = r.truck_and_routing_classes.len().min(250);
        s.push_str(&format!(
            "    {{ \"class_name\": {}, \"members\": {} }}{}\n",
            json_escape(cls),
            json_str_array(mems),
            if idx + 1 < len { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");
    s.push_str(&format!(
        "  \"truck_and_routing_classes_total\": {},\n",
        r.truck_and_routing_classes.len()
    ));

    s.push_str(&format!(
        "  \"crypto_methods\": {},\n",
        serialize_disassembled_methods(&r.crypto_methods)
    ));
    s.push_str(&format!(
        "  \"network_and_socket_methods\": {},\n",
        serialize_disassembled_methods(&r.network_and_socket_methods)
    ));
    s.push_str(&format!(
        "  \"database_and_pref_methods\": {},\n",
        serialize_disassembled_methods(&r.database_and_pref_methods)
    ));
    s.push_str(&format!(
        "  \"domain_core_methods\": {},\n",
        serialize_disassembled_methods(&r.domain_core_methods)
    ));

    s.push_str(&format!(
        "  \"discovered_urls\": {},\n",
        json_str_array(&r.discovered_urls)
    ));
    s.push_str(&format!(
        "  \"sql_statements\": {},\n",
        json_str_array(&r.sql_statements)
    ));
    s.push_str(&format!(
        "  \"shared_prefs_names\": {}\n",
        json_str_array(&r.shared_prefs_names)
    ));

    s.push_str("}\n");
    s
}

fn serialize_components(list: &[ComponentEntry]) -> String {
    let mut s = String::from("[\n");
    for (idx, c) in list.iter().enumerate() {
        let exp = match c.exported {
            Some(b) => b.to_string(),
            None => "null".to_string(),
        };
        let perm = match &c.permission {
            Some(p) => json_escape(p),
            None => "null".to_string(),
        };
        let fgs = match &c.foreground_service_type {
            Some(f) => json_escape(f),
            None => "null".to_string(),
        };
        let auth = match &c.authorities {
            Some(a) => json_escape(a),
            None => "null".to_string(),
        };
        let proc_s = match &c.process {
            Some(p) => json_escape(p),
            None => "null".to_string(),
        };
        s.push_str(&format!(
            "    {{ \"kind\": {}, \"name\": {}, \"exported\": {}, \"permission\": {}, \"foreground_service_type\": {}, \"authorities\": {}, \"process\": {}, \"actions\": {}, \"categories\": {}, \"deeplinks\": {} }}{}\n",
            json_escape(c.kind),
            json_escape(&c.name),
            exp,
            perm,
            fgs,
            auth,
            proc_s,
            json_str_array(&c.actions),
            json_str_array(&c.categories),
            json_str_array(&c.deeplinks),
            if idx + 1 < list.len() { "," } else { "" }
        ));
    }
    s.push_str("  ]");
    s
}

pub fn axml_report_to_json(r: &AxmlManifestReport) -> String {
    let mut s = String::with_capacity(32 * 1024);
    let ct = match r.uses_cleartext_traffic {
        Some(b) => b.to_string(),
        None => "null".to_string(),
    };
    let ab = match r.allow_backup {
        Some(b) => b.to_string(),
        None => "null".to_string(),
    };
    s.push_str("{\n");
    s.push_str(&format!("  \"package_name\": {},\n", json_escape(&r.package_name)));
    s.push_str(&format!("  \"version_code\": {},\n", json_escape(&r.version_code)));
    s.push_str(&format!("  \"version_name\": {},\n", json_escape(&r.version_name)));
    s.push_str(&format!("  \"min_sdk\": {},\n", json_escape(&r.min_sdk)));
    s.push_str(&format!("  \"target_sdk\": {},\n", json_escape(&r.target_sdk)));
    s.push_str(&format!("  \"compile_sdk\": {},\n", json_escape(&r.compile_sdk)));
    s.push_str(&format!(
        "  \"application_class\": {},\n",
        json_escape(&r.application_class)
    ));
    s.push_str(&format!(
        "  \"app_component_factory\": {},\n",
        json_escape(&r.app_component_factory)
    ));
    s.push_str(&format!("  \"uses_cleartext_traffic\": {},\n", ct));
    s.push_str(&format!("  \"allow_backup\": {},\n", ab));
    s.push_str(&format!(
        "  \"network_security_config\": {},\n",
        json_escape(&r.network_security_config)
    ));
    s.push_str(&format!("  \"permissions\": {},\n", json_str_array(&r.permissions)));
    s.push_str(&format!(
        "  \"custom_permissions\": {},\n",
        json_str_array(&r.custom_permissions)
    ));
    s.push_str(&format!(
        "  \"uses_features\": {},\n",
        json_str_array(&r.uses_features)
    ));
    s.push_str(&format!(
        "  \"queries_packages\": {},\n",
        json_str_array(&r.queries_packages)
    ));
    s.push_str(&format!(
        "  \"queries_schemes\": {},\n",
        json_str_array(&r.queries_schemes)
    ));
    s.push_str(&format!("  \"activities_count\": {},\n", r.activities.len()));
    s.push_str(&format!("  \"services_count\": {},\n", r.services.len()));
    s.push_str(&format!("  \"receivers_count\": {},\n", r.receivers.len()));
    s.push_str(&format!("  \"providers_count\": {},\n", r.providers.len()));
    s.push_str(&format!(
        "  \"activities\": {},\n",
        serialize_components(&r.activities)
    ));
    s.push_str(&format!(
        "  \"services\": {},\n",
        serialize_components(&r.services)
    ));
    s.push_str(&format!(
        "  \"receivers\": {},\n",
        serialize_components(&r.receivers)
    ));
    s.push_str(&format!(
        "  \"providers\": {},\n",
        serialize_components(&r.providers)
    ));
    s.push_str(&format!(
        "  \"all_deeplinks\": {},\n",
        json_str_array(&r.all_deeplinks)
    ));
    s.push_str("  \"meta_data\": [\n");
    for (idx, m) in r.meta_data.iter().enumerate() {
        s.push_str(&format!(
            "    {{ \"name\": {}, \"value_summary\": {} }}{}\n",
            json_escape(&m.name),
            json_escape(&m.value_summary),
            if idx + 1 < r.meta_data.len() { "," } else { "" }
        ));
    }
    s.push_str("  ],\n");
    s.push_str(&format!(
        "  \"xml_preview_lines\": {}\n",
        json_str_array(&r.xml_preview_lines)
    ));
    s.push_str("}\n");
    s
}

pub fn arsc_report_to_json(r: &ArscTableReport) -> String {
    let mut s = String::with_capacity(16 * 1024);
    s.push_str("{\n");
    s.push_str(&format!("  \"file_size\": {},\n", r.file_size));
    s.push_str(&format!(
        "  \"global_string_count\": {},\n",
        r.global_string_count
    ));
    s.push_str(&format!(
        "  \"package_names\": {},\n",
        json_str_array(&r.package_names)
    ));
    s.push_str("  \"type_spec_counts\": {\n");
    let entries: Vec<(&String, &u32)> = r.type_spec_counts.iter().collect();
    for (idx, (k, v)) in entries.iter().enumerate() {
        s.push_str(&format!(
            "    {}: {}{}\n",
            json_escape(k),
            v,
            if idx + 1 < entries.len() { "," } else { "" }
        ));
    }
    s.push_str("  },\n");
    s.push_str(&format!("  \"layout_names\": {},\n", json_str_array(&r.layout_names)));
    s.push_str(&format!(
        "  \"xml_config_names\": {},\n",
        json_str_array(&r.xml_config_names)
    ));
    s.push_str(&format!(
        "  \"notable_domain_strings\": {}\n",
        json_str_array(&r.notable_domain_strings)
    ));
    s.push_str("}\n");
    s
}
