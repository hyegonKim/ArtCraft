//! Dreamweaver (`DRWV v21.8.1`) & Responsive Web Authoring:
//! - Semantic HTML5 + CSS3 Flexbox / Grid site bundle exporter (`file.exportHtmlSite`)
//! - Live HTML5 + CSS3 split preview generator (`code.liveHtmlCss`)
//! - Responsive `@media` queries across `Desktop 1440px`, `Tablet 768px`, and `Mobile 375px`
//!   (diffing multi-frame artboard variants when present, and synthesizing adaptive Flexbox/Grid
//!   reflow rules for single-frame responsive export)
//! - CSS custom properties (`:root` + alternate variable modes) from FigCraft Variables
//! - Interactive multi-state `:hover` and `:active` rules from component variants (`State=Hover`,
//!   `State=Active`/`Pressed`) and prototype interactions (`OnHover`/`MouseEnter`, `OnPress`/`MouseDown`).

use std::collections::{BTreeMap, BTreeSet};

use base64::Engine as _;
use figcraft_color::{Gradient, GradientKind, PaintKind};
use figcraft_doc::proto::{Action, Easing, Trigger, walk};
use figcraft_doc::{
    AutoLayout, ContentAlign, CrossAlign, Document, EffectKind, GridTrack, LayoutMode, LetterSpacing, LineHeight, ListStyle, MainAlign, Node, NodeId,
    NodeKind, Sizing, TextAlignH, TextCase, TextData, TextDecoration, Variable, VariableValue, trim_num,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cmd::CommandSpec;
use crate::codegen::{CssNames, Names, class_name, num, px_to_rem};
use crate::components::{default_variant, variant_props, variant_set};
use crate::library::local_collections;
use crate::params::{boolean, get, id_of, num_or, string, targets};
use crate::{EngineError, Result, Session, bad, spec};

/// Default responsive viewport tiers (`Mobile 375px`, `Tablet 768px`, `Desktop 1440px`).
pub const DEFAULT_BREAKPOINTS: [(u32, &str); 3] = [(375, "Mobile"), (768, "Tablet"), (1440, "Desktop")];

const MAX_EXPORT_DEPTH: usize = 16;
const MAX_EXPORT_CHILDREN: usize = 250;

/// Options controlling HTML5 + CSS3 site generation.
#[derive(Clone, Debug)]
pub struct SiteOptions {
    /// Target responsive widths in ascending order (e.g. `[375, 768, 1440]`).
    pub breakpoints: Vec<BreakpointTier>,
    /// Emit `:root` and mode custom properties from FigCraft Variables.
    pub include_variables: bool,
    /// Convert `px` lengths in CSS to `rem` of `root_font_size` when `Some`.
    pub rem_root: Option<f64>,
    /// Document `<title>`.
    pub title: Option<String>,
    /// Inline `<style>` inside `index.html` instead of linking `styles.css`.
    pub inline_css: bool,
}

impl Default for SiteOptions {
    fn default() -> Self {
        Self {
            breakpoints: DEFAULT_BREAKPOINTS.iter().map(|(w, name)| BreakpointTier { name: (*name).to_string(), width: *w, frame: None }).collect(),
            include_variables: true,
            rem_root: None,
            title: None,
            inline_css: false,
        }
    }
}

/// One responsive breakpoint tier (`Mobile 375px`, `Tablet 768px`, `Desktop 1440px`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakpointTier {
    pub name: String,
    pub width: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<NodeId>,
}

/// One file produced by `file.exportHtmlSite`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SiteAssetFile {
    pub name: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Generated semantic HTML5 + CSS3 site bundle.
#[derive(Clone, Debug, Default, Serialize)]
pub struct GeneratedSite {
    pub title: String,
    pub html: String,
    pub css: String,
    pub combined: String,
    pub breakpoints: Vec<BreakpointTier>,
    pub variables_count: usize,
    pub states_count: usize,
    #[serde(skip)]
    pub image_files: Vec<(String, Vec<u8>)>,
}

type PathRules = BTreeMap<String, (String, Vec<(String, String)>, NodeMeta)>;
type ResolvedTargets = (Vec<NodeId>, Vec<(u32, String, NodeId)>, Vec<BreakpointTier>);

struct EmitCtx<'a> {
    doc: &'a Document,
    var_names: CssNames,
    used_classes: Names,
    bound_vars: Vec<(String, String)>,
    css_blocks: Vec<String>,
    state_blocks: Vec<String>,
    path_rules: PathRules,
    images: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct NodeMeta {
    is_flex_row: bool,
    child_count: usize,
    grid_cols: u32,
    width: f64,
    font_size: Option<f64>,
    padding_h: f64,
    is_root: bool,
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "code.liveHtmlCss",
            "Live HTML5 + CSS3",
            "{ids?, breakpoints?, includeVariables?, unit?, rootFontSize?, title?} → {html, css, combined, breakpoints, variables, states}  responsive semantic HTML5 + CSS3 Flexbox/Grid, FigCraft Variables as CSS custom properties, and :hover/:active interactive states",
            false,
            run_live_html_css
        ),
        spec!(
            "file.exportHtmlSite",
            "Export responsive HTML site",
            "{ids?, dir?, path?, inlineCss?, title?, breakpoints?, includeVariables?, unit?, rootFontSize?} → {files: [{name, bytes, path?, content?}], html, css, combined, breakpoints, data?}  Dreamweaver-style semantic HTML5 + CSS3 site bundle (`index.html`, `styles.css`, image assets) or standalone HTML file",
            false,
            run_export_html_site
        ),
        spec!(
            "file.importHtml",
            "Import HTML",
            "{html? | path? | data? (base64), name?, x?, y?, parent?, width?} → {id, ids, layers, warnings}  imports semantic HTML5 + inline/`<style>` CSS into editable FigCraft frames, text layers and rectangles",
            true,
            super::dreamweaver_import::import_html
        ),
    ]
}

fn run_live_html_css(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "code.liveHtmlCss";
    let opts = parse_site_options(CMD, s, p)?;
    let ids = resolve_export_targets(CMD, s, p)?;
    let doc = s.doc()?;
    let site = generate_site(doc, &ids, &opts);
    Ok(json!({
        "html": site.html,
        "css": site.css,
        "combined": site.combined,
        "breakpoints": site.breakpoints,
        "variables": site.variables_count,
        "states": site.states_count,
    }))
}

fn run_export_html_site(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "file.exportHtmlSite";
    let mut opts = parse_site_options(CMD, s, p)?;
    let dir = string(p, "dir");
    let path = string(p, "path");
    if path.is_some() && boolean(p, "inlineCss").is_none() {
        opts.inline_css = true;
    }
    let ids = resolve_export_targets(CMD, s, p)?;
    let doc = s.doc()?;
    let site = generate_site(doc, &ids, &opts);

    if let Some(dir_path) = dir {
        return write_site_dir(&dir_path, &site, opts.inline_css);
    }
    if let Some(file_path) = path {
        return write_site_file(&file_path, &site, opts.inline_css);
    }

    let index_html = if opts.inline_css { site.combined.clone() } else { external_html_doc(&site.title, &site.html, "styles.css") };
    let mut files = vec![json!({
        "name": "index.html",
        "bytes": index_html.len(),
        "content": index_html,
    })];
    if !opts.inline_css {
        files.push(json!({
            "name": "styles.css",
            "bytes": site.css.len(),
            "content": site.css,
        }));
    }
    for (rel, bytes) in &site.image_files {
        files.push(json!({
            "name": rel,
            "bytes": bytes.len(),
        }));
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(site.combined.as_bytes());
    Ok(json!({
        "files": files,
        "html": site.html,
        "css": site.css,
        "combined": site.combined,
        "breakpoints": site.breakpoints,
        "variables": site.variables_count,
        "states": site.states_count,
        "data": b64,
    }))
}

fn write_site_dir(dir_path: &str, site: &GeneratedSite, inline_css: bool) -> Result<Value> {
    let base = std::path::Path::new(dir_path);
    crate::file_access::create_dir_all(base)?;
    let mut written = Vec::new();

    let index_content = if inline_css { site.combined.clone() } else { external_html_doc(&site.title, &site.html, "styles.css") };
    let index_path = base.join("index.html");
    crate::file_access::write(&index_path, index_content.as_bytes())?;
    written.push(json!({
        "name": "index.html",
        "bytes": index_content.len(),
        "path": index_path.to_string_lossy(),
    }));

    if !inline_css {
        let css_path = base.join("styles.css");
        crate::file_access::write(&css_path, site.css.as_bytes())?;
        written.push(json!({
            "name": "styles.css",
            "bytes": site.css.len(),
            "path": css_path.to_string_lossy(),
        }));
    }

    if !site.image_files.is_empty() {
        let assets_dir = base.join("assets");
        crate::file_access::create_dir_all(&assets_dir)?;
        for (rel, bytes) in &site.image_files {
            let asset_path = base.join(rel);
            crate::file_access::write(&asset_path, bytes)?;
            written.push(json!({
                "name": rel,
                "bytes": bytes.len(),
                "path": asset_path.to_string_lossy(),
            }));
        }
    }

    Ok(json!({
        "files": written,
        "html": site.html,
        "css": site.css,
        "combined": site.combined,
        "breakpoints": site.breakpoints,
    }))
}

fn write_site_file(file_path: &str, site: &GeneratedSite, inline_css: bool) -> Result<Value> {
    let target = std::path::Path::new(file_path);
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        crate::file_access::create_dir_all(parent)?;
    }
    let mut written = Vec::new();
    let html_out = if inline_css { site.combined.clone() } else { external_html_doc(&site.title, &site.html, "styles.css") };
    crate::file_access::write(target, html_out.as_bytes())?;
    written.push(json!({
        "name": target.file_name().and_then(std::ffi::OsStr::to_str).unwrap_or("index.html"),
        "bytes": html_out.len(),
        "path": target.to_string_lossy(),
    }));
    if !inline_css && let Some(parent) = target.parent() {
        let css_path = parent.join("styles.css");
        crate::file_access::write(&css_path, site.css.as_bytes())?;
        written.push(json!({
            "name": "styles.css",
            "bytes": site.css.len(),
            "path": css_path.to_string_lossy(),
        }));
    }
    Ok(json!({
        "files": written,
        "html": site.html,
        "css": site.css,
        "combined": site.combined,
        "breakpoints": site.breakpoints,
    }))
}

fn parse_site_options(cmd: &str, s: &Session, p: &Value) -> Result<SiteOptions> {
    let mut opts = SiteOptions::default();
    if let Some(inc) = boolean(p, "includeVariables") {
        opts.include_variables = inc;
    }
    if let Some(inl) = boolean(p, "inlineCss") {
        opts.inline_css = inl;
    }
    opts.title = string(p, "title").filter(|t| !t.trim().is_empty());
    opts.rem_root = match string(p, "unit").map(|u| u.to_ascii_lowercase()).as_deref() {
        None | Some("px") => None,
        Some("rem") => {
            let root = num_or(p, "rootFontSize", 16.0);
            if !(root.is_finite() && root > 0.0 && root <= 1000.0) {
                return Err(bad(cmd, "rootFontSize: a positive number of pixels (up to 1000)"));
            }
            Some(root)
        }
        Some(_) => return Err(bad(cmd, "unit: px|rem")),
    };
    if let Some(bp_val) = get(p, "breakpoints") {
        opts.breakpoints = parse_breakpoints_param(cmd, s.doc()?, bp_val)?;
    }
    Ok(opts)
}

fn parse_breakpoints_param(cmd: &str, doc: &Document, val: &Value) -> Result<Vec<BreakpointTier>> {
    match val {
        Value::Null | Value::Bool(true) => Ok(SiteOptions::default().breakpoints),
        Value::Bool(false) => Ok(Vec::new()),
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len().min(12));
            for item in arr.iter().take(12) {
                if let Some(w) = item.as_u64().filter(|w| (120..=7680).contains(w)) {
                    let width = w as u32;
                    out.push(BreakpointTier { name: tier_label(width), width, frame: None });
                } else if let Some(obj) = item.as_object() {
                    let width = obj
                        .get("width")
                        .and_then(Value::as_u64)
                        .filter(|w| (120..=7680).contains(w))
                        .ok_or_else(|| bad(cmd, "breakpoints: width must be 120..=7680"))? as u32;
                    let name = obj.get("name").and_then(Value::as_str).map_or_else(|| tier_label(width), str::to_string);
                    let frame = match obj.get("id").or_else(|| obj.get("frame")) {
                        Some(v) if !v.is_null() => {
                            Some(id_of(v).filter(|id| doc.contains(*id)).ok_or_else(|| bad(cmd, "breakpoints: unknown frame id"))?)
                        }
                        _ => None,
                    };
                    out.push(BreakpointTier { name, width, frame });
                } else {
                    return Err(bad(cmd, "breakpoints: array of widths or {name?, width, id?}"));
                }
            }
            out.sort_by_key(|b| b.width);
            out.dedup_by_key(|b| b.width);
            Ok(out)
        }
        _ => Err(bad(cmd, "breakpoints: array of widths or booleans")),
    }
}

fn tier_label(width: u32) -> String {
    match width {
        0..=599 => "Mobile".into(),
        600..=1023 => "Tablet".into(),
        _ => "Desktop".into(),
    }
}

fn resolve_export_targets(cmd: &str, s: &Session, p: &Value) -> Result<Vec<NodeId>> {
    let doc = s.doc()?;
    let chosen: Vec<NodeId> = targets(s, p).into_iter().filter(|id| doc.contains(*id) && !doc.node(*id).is_some_and(Node::is_page)).collect();
    if !chosen.is_empty() {
        return Ok(chosen);
    }
    let page = s.page()?;
    let kids: Vec<NodeId> =
        doc.node(page).map(|pg| pg.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| n.visible)).collect()).unwrap_or_default();
    if kids.is_empty() {
        return Err(bad(cmd, "select a frame or layer, or add layers to the current page"));
    }
    Ok(kids)
}

/// Generate a complete responsive HTML5 + CSS3 site from `ids` in `doc`.
pub fn generate_site(doc: &Document, ids: &[NodeId], opts: &SiteOptions) -> GeneratedSite {
    let roots = effective_roots(doc, ids);
    if roots.is_empty() {
        let title = opts.title.clone().unwrap_or_else(|| doc.name.clone());
        let combined = inline_html_doc(&title, "", "");
        return GeneratedSite { title, combined, breakpoints: opts.breakpoints.clone(), ..GeneratedSite::default() };
    }

    let (primary_roots, variant_frames, resolved_bps) = group_responsive_variants(doc, &roots, &opts.breakpoints);

    let mut ctx = EmitCtx {
        doc,
        var_names: CssNames::new(doc),
        used_classes: Names::default(),
        bound_vars: Vec::new(),
        css_blocks: Vec::new(),
        state_blocks: Vec::new(),
        path_rules: BTreeMap::new(),
        images: BTreeMap::new(),
    };

    let mut html_body = String::new();
    let multi = primary_roots.len() > 1;
    if multi {
        html_body.push_str("<main class=\"dw-page\">\n");
    }
    for (idx, &root_id) in primary_roots.iter().enumerate() {
        let path = if multi { format!("/root#{idx}") } else { String::new() };
        emit_node(&mut ctx, root_id, None, usize::from(multi), &path, true, &mut html_body);
    }
    if multi {
        html_body.push_str("</main>\n");
    }

    let (vars_css, vars_count) = build_variables_css(doc, &ctx.var_names, &ctx.bound_vars, opts.include_variables);
    let media_css = build_media_queries(doc, &mut ctx, &variant_frames, &resolved_bps);

    let mut css = String::new();
    css.push_str("/* Dreamweaver Responsive HTML5 + CSS3 Stylesheet */\n");
    css.push_str("*, *::before, *::after {\n  box-sizing: border-box;\n}\n\n");
    css.push_str("body {\n  margin: 0;\n  min-height: 100vh;\n  font-family: Inter, system-ui, -apple-system, sans-serif;\n}\n\n");
    if multi {
        css.push_str(".dw-page {\n  display: flex;\n  flex-direction: column;\n  align-items: center;\n  width: 100%;\n}\n\n");
    }
    if !vars_css.is_empty() {
        css.push_str(&vars_css);
        css.push('\n');
    }
    for block in &ctx.css_blocks {
        css.push_str(block);
        css.push('\n');
    }
    let states_count = ctx.state_blocks.len();
    if !ctx.state_blocks.is_empty() {
        css.push_str("/* Interactive States (:hover / :active) */\n");
        for block in &ctx.state_blocks {
            css.push_str(block);
            css.push('\n');
        }
    }
    if !media_css.is_empty() {
        css.push_str(&media_css);
    }

    if let Some(root_px) = opts.rem_root {
        css = px_to_rem(&css, root_px);
    }

    let image_files: Vec<(String, Vec<u8>)> =
        ctx.images.iter().filter_map(|(hash, rel)| doc.images.get(hash).map(|img| (rel.clone(), (*img.bytes).clone()))).collect();

    let title = opts
        .title
        .clone()
        .or_else(|| primary_roots.first().and_then(|id| doc.node(*id)).map(|n| n.name.clone()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| doc.name.clone());
    let safe_css = css.replace("</", "<\\/");
    let combined = inline_html_doc(&title, &html_body, &safe_css);

    GeneratedSite { title, html: html_body, css, combined, breakpoints: resolved_bps, variables_count: vars_count, states_count, image_files }
}

/// CSS slice filtered for a specific responsive viewport width (`None` returns the full stylesheet).
pub fn css_for_breakpoint(site: &GeneratedSite, width: Option<u32>) -> String {
    let Some(target_w) = width else { return site.css.clone() };
    // Keep base rules + `:root` + `:hover`/`:active` + `@media` blocks that apply at `target_w`.
    let mut out = String::with_capacity(site.css.len());
    let mut i = 0;
    let bytes = site.css.as_bytes();
    while i < bytes.len() {
        let Some(media_rel) = site.css[i..].find("@media ") else {
            out.push_str(&site.css[i..]);
            break;
        };
        out.push_str(&site.css[i..i + media_rel]);
        let start = i + media_rel;
        let Some(open_rel) = site.css[start..].find('{') else {
            out.push_str(&site.css[start..]);
            break;
        };
        let header = &site.css[start..start + open_rel];
        let mut depth = 1usize;
        let mut cur = start + open_rel + 1;
        while cur < bytes.len() && depth > 0 {
            match bytes[cur] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            cur += 1;
        }
        let end = if cur < bytes.len() && bytes[cur] == b'\n' { cur + 1 } else { cur };
        if media_header_matches(header, target_w) {
            out.push_str(&site.css[start..end]);
        }
        i = end;
    }
    out
}

fn media_header_matches(header: &str, viewport_w: u32) -> bool {
    if header.contains("prefers-color-scheme") {
        return true;
    }
    if let Some(idx) = header.find("max-width:") {
        let digits: String = header[idx + 10..].trim_start().chars().take_while(char::is_ascii_digit).collect();
        if let Ok(max_w) = digits.parse::<u32>() {
            return viewport_w <= max_w;
        }
    }
    if let Some(idx) = header.find("min-width:") {
        let digits: String = header[idx + 10..].trim_start().chars().take_while(char::is_ascii_digit).collect();
        if let Ok(min_w) = digits.parse::<u32>() {
            return viewport_w >= min_w;
        }
    }
    true
}

fn effective_roots(doc: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    if !ids.is_empty() {
        return ids.iter().copied().filter(|id| doc.node(*id).is_some_and(|n| !n.is_page())).collect();
    }
    let Some(page) = doc.first_page() else { return Vec::new() };
    doc.node(page).map(|pg| pg.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| n.visible)).collect()).unwrap_or_default()
}

/// Detect whether `roots` (or explicit `breakpoints` frame IDs, or same-stem sibling frames on the
/// page) represent responsive breakpoint variants of the same screen (e.g. `1440px` Desktop +
/// `768px` Tablet + `375px` Mobile).
fn group_responsive_variants(doc: &Document, roots: &[NodeId], requested: &[BreakpointTier]) -> ResolvedTargets {
    let mut tiers = requested.to_vec();

    // 1. Explicit frame IDs passed in `breakpoints`.
    if tiers.iter().any(|t| t.frame.is_some()) {
        let widest = tiers.iter().rev().find_map(|t| t.frame).or_else(|| roots.first().copied());
        let primary: Vec<NodeId> = widest.into_iter().collect();
        let variants: Vec<(u32, String, NodeId)> =
            tiers.iter().filter_map(|t| t.frame.filter(|f| Some(*f) != widest).map(|f| (t.width, t.name.clone(), f))).collect();
        return (primary, variants, tiers);
    }

    // 2. Multiple selected frame-like roots with distinct responsive widths (e.g. Desktop 1440 + Tablet 768 + Mobile 375).
    let all_frames = roots.len() >= 2 && roots.iter().all(|id| doc.node(*id).is_some_and(|n| n.kind.is_frame_like()));
    if all_frames && looks_like_breakpoint_set(doc, roots) {
        let mut by_width: Vec<(NodeId, u32)> = roots.iter().filter_map(|&id| doc.node(id).map(|n| (id, n.width.round().max(1.0) as u32))).collect();
        by_width.sort_by_key(|x| std::cmp::Reverse(x.1));
        let (desktop_id, desktop_w) = by_width[0];
        let mut variants = Vec::new();
        let mut built_tiers = Vec::new();
        for &(fid, w) in by_width.iter().rev() {
            let label = tier_label(w);
            built_tiers.push(BreakpointTier { name: label.clone(), width: w, frame: Some(fid) });
            if fid != desktop_id {
                variants.push((w, label, fid));
            }
        }
        // Keep standard tiers that weren't supplied as separate frames so synthesized rules still cover them.
        for std_bp in &tiers {
            if !built_tiers.iter().any(|b| b.width.abs_diff(std_bp.width) <= 60) && std_bp.width < desktop_w {
                built_tiers.push(std_bp.clone());
            }
        }
        built_tiers.sort_by_key(|b| b.width);
        return (vec![desktop_id], variants, built_tiers);
    }

    // 3. Single root selected: check if sibling frames on its parent share the same base screen name
    // (e.g. "Landing - Desktop", "Landing - Tablet", "Landing - Mobile").
    if let [only] = roots
        && let Some(n) = doc.node(*only)
        && n.kind.is_frame_like()
        && let Some(stem) = responsive_stem(&n.name)
        && let Some(parent_node) = n.parent.and_then(|p| doc.node(p))
    {
        let mut family: Vec<(NodeId, u32)> = parent_node
            .children
            .iter()
            .copied()
            .filter_map(|cid| {
                let cn = doc.node(cid)?;
                (cn.visible && cn.kind.is_frame_like() && responsive_stem(&cn.name).as_deref() == Some(stem.as_str()))
                    .then_some((cid, cn.width.round().max(1.0) as u32))
            })
            .collect();
        family.sort_by_key(|x| std::cmp::Reverse(x.1));
        family.dedup_by_key(|(_, w)| *w);
        if family.len() >= 2 {
            let (desktop_id, _) = family[0];
            let mut variants = Vec::new();
            for &(fid, w) in &family[1..] {
                variants.push((w, tier_label(w), fid));
            }
            for t in &mut tiers {
                if let Some(&(fid, _)) = family.iter().find(|(_, w)| w.abs_diff(t.width) <= 80) {
                    t.frame = Some(fid);
                }
            }
            return (vec![desktop_id], variants, tiers);
        }
    }

    if let [only] = roots
        && let Some(n) = doc.node(*only)
    {
        let w = n.width.round().max(1.0) as u32;
        for t in &mut tiers {
            if t.width.abs_diff(w) <= 80 {
                t.frame = Some(*only);
            }
        }
    }
    (roots.to_vec(), Vec::new(), tiers)
}

fn looks_like_breakpoint_set(doc: &Document, roots: &[NodeId]) -> bool {
    let widths: Vec<u32> = roots.iter().filter_map(|id| doc.node(*id).map(|n| n.width.round().max(1.0) as u32)).collect();
    if widths.len() != roots.len() || widths.len() > 5 {
        return false;
    }
    let min_w = widths.iter().copied().min().unwrap_or(0);
    let max_w = widths.iter().copied().max().unwrap_or(0);
    if max_w < min_w + 180 {
        return false;
    }
    // Either share a common screen stem, or have canonical mobile/tablet/desktop names or widths.
    let stems: Vec<Option<String>> = roots.iter().filter_map(|id| doc.node(*id)).map(|n| responsive_stem(&n.name)).collect();
    if let Some(Some(first)) = stems.first()
        && stems.iter().all(|s| s.as_ref() == Some(first))
    {
        return true;
    }
    widths
        .iter()
        .all(|w| [320u32, 375, 390, 414, 430, 744, 768, 834, 1024, 1280, 1366, 1440, 1512, 1920].iter().any(|std_w| w.abs_diff(*std_w) <= 48))
}

fn responsive_stem(name: &str) -> Option<String> {
    let lower = name.trim().to_ascii_lowercase();
    for suffix in ["desktop", "tablet", "mobile", "phone", "1440px", "1440", "1280", "1024", "768px", "768", "390", "375px", "375", "320"] {
        if let Some(head) = lower.strip_suffix(suffix) {
            let trimmed = head.trim_end_matches([' ', '-', '/', '_', '·', ':']).trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

// ------------------------------------------------------------------ HTML5 + CSS3 tree walk

fn emit_node(ctx: &mut EmitCtx<'_>, id: NodeId, parent: Option<&Node>, depth: usize, path: &str, is_root: bool, markup: &mut String) {
    let Some(n) = ctx.doc.node(id) else { return };
    if !n.visible && !is_root {
        return;
    }
    let cls = class_name(&n.name, kind_fallback(n), &mut ctx.used_classes);
    let pad = "  ".repeat(depth);

    let kids = visible_children(ctx.doc, n);
    let (tag, link_href) = infer_semantic_tag(n, parent, !kids.is_empty());

    let mut decls = compute_node_decls(ctx, n, parent, is_root, tag == "a" || tag == "button");
    let state_rules = collect_interactive_states(ctx, n, &cls, &decls);
    if (!state_rules.is_empty() || tag == "button" || tag == "a") && !decls.iter().any(|(k, _)| k == "transition") {
        let tr_val = interaction_transition_css(n).unwrap_or_else(|| "all 180ms ease-out".into());
        decls.push(("transition".into(), tr_val));
    }
    for rule in state_rules {
        ctx.state_blocks.push(rule);
    }

    let meta = NodeMeta {
        is_flex_row: n.layout.as_ref().is_some_and(|l| l.mode == LayoutMode::Horizontal),
        child_count: kids.len(),
        grid_cols: n.layout.as_ref().map_or(0, |l| if l.mode == LayoutMode::Grid { l.grid_columns.max(1) } else { 0 }),
        width: n.width,
        font_size: n.kind.text().map(|t| t.style.font_size),
        padding_h: n.layout.as_ref().map_or(0.0, |l| l.padding[1].max(l.padding[3])),
        is_root,
    };

    ctx.css_blocks.push(format_rule(&format!(".{cls}"), &decls, ""));
    ctx.path_rules.insert(path.to_string(), (cls.clone(), decls, meta));

    let extra_attrs = build_open_attrs(tag, link_href.as_deref(), n);

    if let NodeKind::Text(tx) = &n.kind {
        emit_text_markup(tx, tag, &cls, &extra_attrs, &pad, markup);
        return;
    }

    if tag == "img" {
        let src = first_image_asset(ctx, n).unwrap_or_else(|| "assets/image.png".into());
        let alt = escape_attr(&n.name);
        markup.push_str(&format!("{pad}<img class=\"{cls}\" src=\"{src}\" alt=\"{alt}\"{extra_attrs} />\n"));
        return;
    }

    if kids.is_empty() || depth >= MAX_EXPORT_DEPTH {
        markup.push_str(&format!("{pad}<{tag} class=\"{cls}\"{extra_attrs}></{tag}>\n"));
        return;
    }

    markup.push_str(&format!("{pad}<{tag} class=\"{cls}\"{extra_attrs}>\n"));
    let mut seen: Vec<&str> = Vec::new();
    for child_id in kids {
        let Some(cn) = ctx.doc.node(child_id) else { continue };
        let occ = seen.iter().filter(|s| **s == cn.name).count();
        seen.push(&cn.name);
        let child_path = format!("{path}/{}#{occ}", cn.name);
        emit_node(ctx, child_id, Some(n), depth + 1, &child_path, false, markup);
    }
    markup.push_str(&format!("{pad}</{tag}>\n"));
}

fn emit_text_markup(tx: &TextData, tag: &str, cls: &str, extra_attrs: &str, pad: &str, markup: &mut String) {
    if matches!(tx.list, ListStyle::Bulleted | ListStyle::Numbered) {
        let list_tag = if tx.list == ListStyle::Numbered { "ol" } else { "ul" };
        markup.push_str(&format!("{pad}<{list_tag} class=\"{cls}\"{extra_attrs}>\n"));
        for line in tx.characters.split('\n') {
            markup.push_str(&format!("{pad}  <li>{}</li>\n", escape_html(line)));
        }
        markup.push_str(&format!("{pad}</{list_tag}>\n"));
        return;
    }
    let body = escape_html(&tx.characters).replace('\n', "<br />");
    markup.push_str(&format!("{pad}<{tag} class=\"{cls}\"{extra_attrs}>{body}</{tag}>\n"));
}

fn visible_children(doc: &Document, n: &Node) -> Vec<NodeId> {
    n.children.iter().copied().filter(|id| doc.node(*id).is_some_and(|c| c.visible)).take(MAX_EXPORT_CHILDREN).collect()
}

fn infer_semantic_tag(n: &Node, parent: Option<&Node>, has_children: bool) -> (&'static str, Option<String>) {
    // Check OpenUrl prototype interaction or text hyperlink first.
    for it in &n.interactions {
        for act in walk(&it.actions) {
            if let Action::OpenUrl { url, .. } = act {
                return ("a", Some(url.clone()));
            }
        }
    }
    if let NodeKind::Text(tx) = &n.kind {
        if let Some(link) = tx.style.link.as_ref().filter(|l| !l.is_empty()) {
            return ("a", Some(link.clone()));
        }
        return (infer_text_tag(n, tx, parent), None);
    }

    let lower = n.name.trim().to_ascii_lowercase();
    if parent.is_some_and(|p| has_list_role(&p.name)) {
        return ("li", None);
    }
    if !has_children && n.fills.iter().any(|f| f.visible && matches!(f.kind, PaintKind::Image(_))) {
        return ("img", None);
    }
    let is_clickable = n.interactions.iter().any(|it| matches!(it.trigger, Trigger::OnClick | Trigger::OnPress | Trigger::MouseDown));
    if is_clickable || word_matches(&lower, &["button", "btn", "cta"]) {
        return ("button", None);
    }
    if word_matches(&lower, &["header", "topbar", "navbar-header", "masthead"]) {
        return ("header", None);
    }
    if word_matches(&lower, &["nav", "navbar", "navigation", "menubar", "breadcrumb"]) {
        return ("nav", None);
    }
    if word_matches(&lower, &["main"]) {
        return ("main", None);
    }
    if word_matches(&lower, &["footer", "bottombar"]) {
        return ("footer", None);
    }
    if word_matches(&lower, &["aside", "sidebar"]) {
        return ("aside", None);
    }
    if word_matches(&lower, &["article", "card", "post"]) {
        return ("article", None);
    }
    if has_list_role(&lower) {
        return ("ul", None);
    }
    if matches!(n.kind, NodeKind::Section { .. })
        || word_matches(&lower, &["section", "hero", "features", "pricing", "testimonials", "faq", "banner"])
    {
        return ("section", None);
    }
    ("div", None)
}

fn infer_text_tag(n: &Node, tx: &TextData, parent: Option<&Node>) -> &'static str {
    if parent.is_some_and(|p| {
        let pl = p.name.to_ascii_lowercase();
        word_matches(&pl, &["button", "btn", "cta", "badge", "chip", "tag"])
    }) {
        return "span";
    }
    let lower = n.name.trim().to_ascii_lowercase();
    if lower == "h1" || lower.starts_with("h1 ") || word_matches(&lower, &["hero-title", "display"]) {
        return "h1";
    }
    if lower == "h2" || lower.starts_with("h2 ") {
        return "h2";
    }
    if lower == "h3" || lower.starts_with("h3 ") {
        return "h3";
    }
    if lower == "h4" || lower.starts_with("h4 ") {
        return "h4";
    }
    let sz = tx.style.font_size;
    let bold = tx.style.weight() >= 600;
    if sz >= 36.0 {
        "h1"
    } else if sz >= 28.0 {
        "h2"
    } else if sz >= 22.0 && bold {
        "h3"
    } else if sz >= 18.0 && bold {
        "h4"
    } else {
        "p"
    }
}

fn has_list_role(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    word_matches(&lower, &["ul", "list", "menu-list", "nav-links"])
}

fn word_matches(lower: &str, candidates: &[&str]) -> bool {
    let tokens: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|s| !s.is_empty()).collect();
    candidates.iter().any(|cand| {
        lower == *cand || lower.replace(' ', "-") == *cand || tokens.first().copied() == Some(*cand) || tokens.last().copied() == Some(*cand)
    })
}

fn build_open_attrs(tag: &str, href: Option<&str>, n: &Node) -> String {
    let mut attrs = String::new();
    if tag == "a" {
        let target_url = href.unwrap_or("#");
        attrs.push_str(&format!(" href=\"{}\"", escape_attr(target_url)));
    } else if tag == "button" {
        attrs.push_str(" type=\"button\"");
    }
    let _ = n;
    attrs
}

fn kind_fallback(n: &Node) -> &'static str {
    match &n.kind {
        NodeKind::Frame | NodeKind::Section { .. } => "section",
        NodeKind::Text(_) => "text",
        NodeKind::Component { .. } | NodeKind::ComponentSet { .. } | NodeKind::Instance { .. } => "component",
        NodeKind::Ellipse { .. } => "ellipse",
        _ => "box",
    }
}

// ------------------------------------------------------------------ CSS3 declaration generation

fn compute_node_decls(ctx: &mut EmitCtx<'_>, n: &Node, parent: Option<&Node>, is_root: bool, interactive_tag: bool) -> Vec<(String, String)> {
    let mut decls = Vec::new();
    let in_flow = parent.is_some_and(|p| p.layout.is_some()) && !n.child_layout.absolute;

    // 1. Placement in parent (Flexbox item, Grid item, or absolute coordinates).
    if is_root {
        decls.push(("position".into(), "relative".into()));
        decls.push(("margin-left".into(), "auto".into()));
        decls.push(("margin-right".into(), "auto".into()));
    } else if in_flow {
        if let Some(pl) = parent.and_then(|p| p.layout.as_ref()) {
            append_flow_child_decls(n, pl, &mut decls);
        }
    } else {
        let c = n.transform.as_coeffs();
        decls.push(("position".into(), "absolute".into()));
        decls.push(("left".into(), format!("{}px", num(c[4]))));
        decls.push(("top".into(), format!("{}px", num(c[5]))));
    }

    // 2. Box sizing (`width`, `max-width`, `height`, `min-height`, min/max constraints).
    append_size_decls(ctx, n, is_root, in_flow, &decls.clone(), &mut decls);

    // 3. Container layout: CSS3 Flexbox or CSS3 Grid.
    if let Some(l) = &n.layout {
        append_container_layout_decls(ctx, n, l, &mut decls);
    } else if !n.children.is_empty() && !is_root && in_flow {
        decls.push(("position".into(), "relative".into()));
    }

    // 4. Visual appearance: overflow, corners, opacity, fills, strokes, shadows/blurs, typography.
    append_visual_decls(ctx, n, interactive_tag, &mut decls);
    decls
}

fn append_flow_child_decls(n: &Node, parent_layout: &AutoLayout, decls: &mut Vec<(String, String)>) {
    if parent_layout.mode == LayoutMode::Grid {
        let c_span = n.child_layout.column_span();
        let r_span = n.child_layout.row_span();
        match (n.child_layout.grid_column, c_span > 1) {
            (Some(col), true) => decls.push(("grid-column".into(), format!("{} / span {c_span}", col + 1))),
            (Some(col), false) => decls.push(("grid-column".into(), format!("{}", col + 1))),
            (None, true) => decls.push(("grid-column".into(), format!("span {c_span}"))),
            (None, false) => {}
        }
        match (n.child_layout.grid_row, r_span > 1) {
            (Some(row), true) => decls.push(("grid-row".into(), format!("{} / span {r_span}", row + 1))),
            (Some(row), false) => decls.push(("grid-row".into(), format!("{}", row + 1))),
            (None, true) => decls.push(("grid-row".into(), format!("span {r_span}"))),
            (None, false) => {}
        }
        if n.child_layout.sizing_h == Sizing::Fill {
            decls.push(("width".into(), "100%".into()));
        }
        return;
    }

    let vertical = parent_layout.mode == LayoutMode::Vertical;
    let (main, cross) =
        if vertical { (n.child_layout.sizing_v, n.child_layout.sizing_h) } else { (n.child_layout.sizing_h, n.child_layout.sizing_v) };
    if main == Sizing::Fill {
        decls.push(("flex".into(), "1 1 0%".into()));
    }
    if cross == Sizing::Fill {
        decls.push(("align-self".into(), "stretch".into()));
    }
}

fn append_size_decls(
    ctx: &mut EmitCtx<'_>,
    n: &Node,
    is_root: bool,
    in_flow: bool,
    existing: &[(String, String)],
    decls: &mut Vec<(String, String)>,
) {
    let px = |v: f64| format!("{}px", num(v));
    let hugs = n.layout.is_some() || matches!(n.kind, NodeKind::Text(_));
    let hug_w = hugs && n.child_layout.sizing_h == Sizing::Hug && !is_root;
    let hug_h = hugs && n.child_layout.sizing_v == Sizing::Hug && !is_root;
    let flex_fill = existing.iter().any(|(k, _)| k == "flex") || (in_flow && n.child_layout.sizing_h == Sizing::Fill);

    if is_root {
        let w_tok = bound_val(ctx, n, &["width"], px(n.width));
        decls.push(("width".into(), "100%".into()));
        decls.push(("max-width".into(), w_tok));
        decls.push(("min-height".into(), bound_val(ctx, n, &["height"], px(n.height))));
    } else {
        if !hug_w && !flex_fill {
            let w_tok = bound_val(ctx, n, &["width"], px(n.width));
            if n.layout.is_some() {
                decls.push(("width".into(), "100%".into()));
                decls.push(("max-width".into(), w_tok));
            } else if !matches!(n.kind, NodeKind::Text(_)) {
                decls.push(("width".into(), w_tok));
            }
        }
        if !hug_h {
            let h_tok = bound_val(ctx, n, &["height"], px(n.height));
            if n.layout.is_some() {
                decls.push(("min-height".into(), h_tok));
            } else if !matches!(n.kind, NodeKind::Text(_)) {
                decls.push(("height".into(), h_tok));
            }
        }
    }

    if let Some(min_w) = n.child_layout.min_w {
        decls.push(("min-width".into(), bound_val(ctx, n, &["minWidth"], px(min_w))));
    }
    if let Some(max_w) = n.child_layout.max_w {
        decls.push(("max-width".into(), bound_val(ctx, n, &["maxWidth"], px(max_w))));
    }
    if let Some(min_h) = n.child_layout.min_h {
        decls.push(("min-height".into(), bound_val(ctx, n, &["minHeight"], px(min_h))));
    }
    if let Some(max_h) = n.child_layout.max_h {
        decls.push(("max-height".into(), bound_val(ctx, n, &["maxHeight"], px(max_h))));
    }
}

fn append_container_layout_decls(ctx: &mut EmitCtx<'_>, n: &Node, l: &AutoLayout, decls: &mut Vec<(String, String)>) {
    let px = |v: f64| format!("{}px", num(v));
    if l.mode == LayoutMode::Grid {
        decls.push(("display".into(), "grid".into()));
        decls.push(("grid-template-columns".into(), format_grid_tracks(&l.column_tracks, l.grid_columns.max(1))));
        if !l.row_tracks.is_empty() && l.row_tracks.iter().any(|t| *t != GridTrack::default()) {
            decls.push(("grid-template-rows".into(), format_grid_tracks(&l.row_tracks, l.grid_rows.max(1))));
        }
        let col_gap = bound_val(ctx, n, &["gap"], px(l.gap));
        let row_gap = bound_val(ctx, n, &["crossGap", "gap"], px(l.cross_gap));
        if col_gap == row_gap {
            decls.push(("gap".into(), col_gap));
        } else {
            decls.push(("gap".into(), format!("{row_gap} {col_gap}")));
        }
    } else {
        decls.push(("display".into(), "flex".into()));
        decls.push(("flex-direction".into(), if l.mode == LayoutMode::Vertical { "column" } else { "row" }.into()));
        if l.wrap {
            decls.push(("flex-wrap".into(), "wrap".into()));
        }
        if !l.main_align.is_auto_gap() {
            let main_gap = bound_val(ctx, n, &["gap"], px(l.gap));
            if l.wrap && (l.cross_gap - l.gap).abs() > 0.01 {
                let cross = bound_val(ctx, n, &["crossGap"], px(l.cross_gap));
                let (rg, cg) = if l.mode == LayoutMode::Horizontal { (cross, main_gap) } else { (main_gap, cross) };
                decls.push(("gap".into(), format!("{rg} {cg}")));
            } else {
                decls.push(("gap".into(), main_gap));
            }
        }
        let justify = match l.main_align {
            MainAlign::Min => "flex-start",
            MainAlign::Center => "center",
            MainAlign::Max => "flex-end",
            MainAlign::SpaceBetween => "space-between",
            MainAlign::SpaceAround => "space-around",
            MainAlign::SpaceEvenly => "space-evenly",
        };
        decls.push(("justify-content".into(), justify.into()));
    }

    let align = match l.cross_align {
        CrossAlign::Min => "flex-start",
        CrossAlign::Center => "center",
        CrossAlign::Max => "flex-end",
        CrossAlign::Baseline => "baseline",
    };
    decls.push(("align-items".into(), align.into()));
    if l.wrap && l.mode != LayoutMode::Grid && l.content_align == ContentAlign::SpaceBetween {
        decls.push(("align-content".into(), "space-between".into()));
    }

    let [pt, pr, pb, pl] = l.padding;
    if pt != 0.0 || pr != 0.0 || pb != 0.0 || pl != 0.0 {
        let t_s = bound_val(ctx, n, &["paddingTop", "paddingVertical", "padding"], px(pt));
        let r_s = bound_val(ctx, n, &["paddingRight", "paddingHorizontal", "padding"], px(pr));
        let b_s = bound_val(ctx, n, &["paddingBottom", "paddingVertical", "padding"], px(pb));
        let l_s = bound_val(ctx, n, &["paddingLeft", "paddingHorizontal", "padding"], px(pl));
        let pad_val = if t_s == r_s && r_s == b_s && b_s == l_s {
            t_s
        } else if t_s == b_s && r_s == l_s {
            format!("{t_s} {r_s}")
        } else {
            format!("{t_s} {r_s} {b_s} {l_s}")
        };
        decls.push(("padding".into(), pad_val));
    }
}

fn format_grid_tracks(tracks: &[GridTrack], count: u32) -> String {
    let n = count.clamp(1, 24) as usize;
    let resolved: Vec<GridTrack> = (0..n).map(|i| tracks.get(i).copied().unwrap_or_default()).collect();
    if resolved.len() > 1 && resolved.iter().all(|t| t.sizing == Sizing::Fill && (t.value - 1.0).abs() < 1e-6) {
        return format!("repeat({}, minmax(0, 1fr))", resolved.len());
    }
    resolved
        .into_iter()
        .map(|t| match t.sizing {
            Sizing::Fixed => format!("{}px", num(t.value.max(0.0))),
            Sizing::Hug => "auto".into(),
            Sizing::Fill => {
                if (t.value - 1.0).abs() < 1e-6 {
                    "minmax(0, 1fr)".into()
                } else {
                    format!("{}fr", num(t.value.max(0.1)))
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn append_visual_decls(ctx: &mut EmitCtx<'_>, n: &Node, interactive_tag: bool, decls: &mut Vec<(String, String)>) {
    let px = |v: f64| format!("{}px", num(v));
    if n.clip && n.kind.is_frame_like() {
        decls.push(("overflow".into(), "hidden".into()));
    }
    if matches!(n.kind, NodeKind::Ellipse { .. }) {
        decls.push(("border-radius".into(), "50%".into()));
    } else if let Some(r) = n.corners.uniform().filter(|r| *r > 0.0) {
        decls.push(("border-radius".into(), bound_val(ctx, n, &["cornerRadius"], px(r))));
    } else if n.corners.radii.iter().any(|r| *r > 0.0) {
        let fields = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        let corners: Vec<String> = n.corners.radii.iter().zip(fields).map(|(r, f)| bound_val(ctx, n, &[f, "cornerRadius"], px(*r))).collect();
        decls.push(("border-radius".into(), corners.join(" ")));
    }
    if n.opacity < 0.999 {
        decls.push(("opacity".into(), bound_val(ctx, n, &["opacity"], num(f64::from(n.opacity)))));
    }

    if let NodeKind::Text(tx) = &n.kind {
        decls.push(("margin".into(), "0".into()));
        for (i, p) in n.fills.iter().enumerate().filter(|(_, p)| p.visible) {
            if let PaintKind::Solid { color } = &p.kind {
                let c = color.with_alpha(color.a * p.opacity).to_css();
                decls.push(("color".into(), if i == 0 { bound_val(ctx, n, &["fill"], c) } else { c }));
            }
        }
        append_typography_decls(ctx, n, tx, decls);
    } else {
        append_background_decls(ctx, n, decls);
    }

    append_border_decls(ctx, n, interactive_tag, decls);
    append_effect_decls(n, decls);

    if interactive_tag || !n.interactions.is_empty() {
        decls.push(("cursor".into(), "pointer".into()));
        if interactive_tag {
            decls.push(("text-decoration".into(), "none".into()));
        }
    }
}

fn append_background_decls(ctx: &mut EmitCtx<'_>, n: &Node, decls: &mut Vec<(String, String)>) {
    let visible: Vec<(usize, &figcraft_color::Paint)> = n.fills.iter().enumerate().filter(|(_, p)| p.visible).collect();
    if visible.is_empty() {
        return;
    }
    if let [(i, p)] = visible.as_slice()
        && let PaintKind::Solid { color } = &p.kind
    {
        let c = color.with_alpha(color.a * p.opacity).to_css();
        decls.push(("background-color".into(), if *i == 0 { bound_val(ctx, n, &["fill"], c) } else { c }));
        return;
    }
    let mut layers = Vec::with_capacity(visible.len());
    for (pos, (i, p)) in visible.iter().enumerate().rev() {
        let layer = match &p.kind {
            PaintKind::Solid { color } => {
                let c = color.with_alpha(color.a * p.opacity).to_css();
                if pos > 0 {
                    format!("linear-gradient(0deg, {c}, {c})")
                } else if *i == 0 {
                    bound_val(ctx, n, &["fill"], c)
                } else {
                    c
                }
            }
            PaintKind::Gradient(g) => format_css_gradient(g, n.width, n.height, p.opacity),
            PaintKind::Image(img) => {
                let rel = register_image_asset(ctx, &img.image, &n.name);
                format!("url(\"{rel}\") center / cover no-repeat")
            }
            PaintKind::Pattern(_) => "url(\"assets/pattern.png\") repeat".into(),
            PaintKind::Video(_) => "url(\"assets/video-poster.png\") center / cover no-repeat".into(),
        };
        layers.push(layer);
    }
    decls.push(("background".into(), layers.join(", ")));
}

fn append_border_decls(ctx: &mut EmitCtx<'_>, n: &Node, interactive_tag: bool, decls: &mut Vec<(String, String)>) {
    let stroke_paint = n.strokes.iter().find(|p| p.visible);
    let Some(paint) = stroke_paint else {
        if interactive_tag {
            decls.push(("border".into(), "none".into()));
        }
        return;
    };
    let PaintKind::Solid { color } = &paint.kind else { return };
    let style = if n.stroke.dashes.is_empty() { "solid" } else { "dashed" };
    let col = bound_val(ctx, n, &["stroke"], color.with_alpha(color.a * paint.opacity).to_css());
    if let Some(sides) = n.stroke.sides {
        for (w, prop) in sides.into_iter().zip(["border-top", "border-right", "border-bottom", "border-left"]) {
            if w > 0.0 {
                decls.push((prop.into(), format!("{}px {style} {col}", num(w))));
            }
        }
    } else if n.stroke.weight > 0.0 {
        let w = bound_val(ctx, n, &["strokeWeight"], format!("{}px", num(n.stroke.weight)));
        decls.push(("border".into(), format!("{w} {style} {col}")));
    }
}

fn append_effect_decls(n: &Node, decls: &mut Vec<(String, String)>) {
    let shadows: Vec<String> = n
        .effects
        .iter()
        .filter(|e| e.visible)
        .filter_map(|e| match &e.kind {
            EffectKind::DropShadow(s) => {
                Some(format!("{}px {}px {}px {}px {}", num(s.offset.0), num(s.offset.1), num(s.radius), num(s.spread), s.color.to_css()))
            }
            EffectKind::InnerShadow(s) => {
                Some(format!("inset {}px {}px {}px {}px {}", num(s.offset.0), num(s.offset.1), num(s.radius), num(s.spread), s.color.to_css()))
            }
            _ => None,
        })
        .collect();
    if !shadows.is_empty() {
        decls.push(("box-shadow".into(), shadows.join(", ")));
    }
    for e in n.effects.iter().filter(|e| e.visible) {
        match &e.kind {
            EffectKind::LayerBlur(b) if !b.is_progressive() => decls.push(("filter".into(), format!("blur({}px)", num(b.radius / 2.0)))),
            EffectKind::BackgroundBlur(b) if !b.is_progressive() => {
                decls.push(("backdrop-filter".into(), format!("blur({}px)", num(b.radius / 2.0))));
            }
            EffectKind::Glass(g) => decls.push(("backdrop-filter".into(), format!("blur({}px)", num(g.sanitized().frost / 2.0)))),
            _ => {}
        }
    }
}

fn append_typography_decls(ctx: &mut EmitCtx<'_>, n: &Node, tx: &TextData, decls: &mut Vec<(String, String)>) {
    let px = |v: f64| format!("{}px", num(v));
    let s = &tx.style;
    let fam = s.font_family.replace('"', "");
    decls.push(("font-family".into(), bound_val(ctx, n, &["fontFamily"], format!("\"{fam}\", sans-serif"))));
    decls.push(("font-size".into(), bound_val(ctx, n, &["fontSize"], px(s.font_size))));
    decls.push(("font-weight".into(), s.weight().to_string()));
    if s.italic() {
        decls.push(("font-style".into(), "italic".into()));
    }
    match s.line_height {
        LineHeight::Pixels(v) => decls.push(("line-height".into(), bound_val(ctx, n, &["lineHeight"], px(v)))),
        LineHeight::Percent(p) => decls.push(("line-height".into(), format!("{}%", num(p)))),
        LineHeight::Auto => {}
    }
    match s.letter_spacing {
        LetterSpacing::Pixels(v) if v != 0.0 => decls.push(("letter-spacing".into(), bound_val(ctx, n, &["letterSpacing"], px(v)))),
        LetterSpacing::Percent(p) if p != 0.0 => decls.push(("letter-spacing".into(), format!("{}em", num(p / 100.0)))),
        _ => {}
    }
    match tx.align_h {
        TextAlignH::Left => {}
        TextAlignH::Center => decls.push(("text-align".into(), "center".into())),
        TextAlignH::Right => decls.push(("text-align".into(), "right".into())),
        TextAlignH::Justified => decls.push(("text-align".into(), "justify".into())),
    }
    match s.case {
        TextCase::Original => {}
        TextCase::Upper => decls.push(("text-transform".into(), "uppercase".into())),
        TextCase::Lower => decls.push(("text-transform".into(), "lowercase".into())),
        TextCase::Title => decls.push(("text-transform".into(), "capitalize".into())),
        TextCase::SmallCaps => decls.push(("font-variant".into(), "small-caps".into())),
    }
    match s.decoration {
        TextDecoration::None => {}
        TextDecoration::Underline => decls.push(("text-decoration".into(), "underline".into())),
        TextDecoration::Strikethrough => decls.push(("text-decoration".into(), "line-through".into())),
    }
}

fn format_css_gradient(g: &Gradient, w: f64, h: f64, opacity: f32) -> String {
    let stops = g
        .sorted_stops()
        .iter()
        .map(|s| format!("{} {}%", s.color.with_alpha(s.color.a * opacity).to_css(), (s.position * 100.0).round()))
        .collect::<Vec<_>>()
        .join(", ");
    let (dx, dy) = ((g.to.0 - g.from.0) * w, (g.to.1 - g.from.1) * h);
    let angle = if dx == 0.0 && dy == 0.0 { 180.0 } else { dx.atan2(-dy).to_degrees().rem_euclid(360.0) };
    match g.kind {
        GradientKind::Linear => format!("linear-gradient({}deg, {stops})", num(angle)),
        GradientKind::Radial | GradientKind::Diamond => format!("radial-gradient(circle, {stops})"),
        GradientKind::Angular => format!("conic-gradient(from {}deg, {stops})", num(angle)),
    }
}

fn bound_val(ctx: &mut EmitCtx<'_>, n: &Node, fields: &[&str], fallback: String) -> String {
    let Some(var) = fields.iter().find_map(|f| crate::devinspect::bound_variable(ctx.doc, n, f)) else {
        return fallback;
    };
    let name = ctx.var_names.name(var);
    if !ctx.bound_vars.iter().any(|(k, _)| *k == name) {
        ctx.bound_vars.push((name.clone(), fallback.clone()));
    }
    format!("var({name}, {fallback})")
}

fn register_image_asset(ctx: &mut EmitCtx<'_>, hash: &str, layer_name: &str) -> String {
    if let Some(existing) = ctx.images.get(hash) {
        return existing.clone();
    }
    let slug = class_name(layer_name, "image", &mut Names::default());
    let short = hash.get(..8).unwrap_or("asset");
    let rel = format!("assets/{slug}-{short}.png");
    ctx.images.insert(hash.to_string(), rel.clone());
    rel
}

fn first_image_asset(ctx: &mut EmitCtx<'_>, n: &Node) -> Option<String> {
    n.fills.iter().filter(|f| f.visible).find_map(|f| match &f.kind {
        PaintKind::Image(img) => Some(register_image_asset(ctx, &img.image, &n.name)),
        _ => None,
    })
}

// ------------------------------------------------------------------ Interactive `:hover` & `:active` states

fn collect_interactive_states(ctx: &mut EmitCtx<'_>, n: &Node, cls: &str, base_decls: &[(String, String)]) -> Vec<String> {
    let mut hover_dest = None;
    let mut active_dest = None;
    let mut has_hover_trigger = false;
    let mut has_press_trigger = false;

    for it in &n.interactions {
        match it.trigger {
            Trigger::OnHover | Trigger::MouseEnter => {
                has_hover_trigger = true;
                for act in walk(&it.actions) {
                    if let Action::ChangeTo { dest } = act {
                        hover_dest = Some(*dest);
                    }
                }
            }
            Trigger::OnPress | Trigger::MouseDown => {
                has_press_trigger = true;
                for act in walk(&it.actions) {
                    if let Action::ChangeTo { dest } = act {
                        active_dest = Some(*dest);
                    }
                }
            }
            _ => {}
        }
    }

    if (hover_dest.is_none() || active_dest.is_none())
        && let Some((vh, va)) = find_state_variants(ctx.doc, n)
    {
        hover_dest = hover_dest.or(vh);
        active_dest = active_dest.or(va);
    }

    let mut rules = Vec::new();
    if let Some(hid) = hover_dest
        && let Some(hn) = ctx.doc.node(hid)
    {
        let h_decls = state_visual_decls(ctx, hn);
        let diff = diff_state_decls(base_decls, &h_decls);
        if !diff.is_empty() {
            rules.push(format_rule(&format!(".{cls}:hover"), &diff, ""));
        }
    } else if has_hover_trigger {
        rules.push(format_rule(&format!(".{cls}:hover"), &[("opacity".into(), "0.92".into())], ""));
    }

    if let Some(aid) = active_dest
        && let Some(an) = ctx.doc.node(aid)
    {
        let a_decls = state_visual_decls(ctx, an);
        let diff = diff_state_decls(base_decls, &a_decls);
        if !diff.is_empty() {
            rules.push(format_rule(&format!(".{cls}:active"), &diff, ""));
        }
    } else if has_press_trigger {
        rules.push(format_rule(&format!(".{cls}:active"), &[("transform".into(), "scale(0.98)".into())], ""));
    }
    rules
}

fn find_state_variants(doc: &Document, n: &Node) -> Option<(Option<NodeId>, Option<NodeId>)> {
    let (set_id, base_id) = match &n.kind {
        NodeKind::ComponentSet { .. } => (n.id, default_variant(doc, n.id)?),
        NodeKind::Component { .. } => (variant_set(doc, n.id)?, n.id),
        NodeKind::Instance { component, .. } => (variant_set(doc, *component)?, *component),
        _ => return None,
    };
    let set_node = doc.node(set_id)?;
    let base_node = doc.node(base_id)?;
    let base_props = variant_props(&base_node.name);
    let mut hover = None;
    let mut active = None;

    for &cid in &set_node.children {
        if cid == base_id {
            continue;
        }
        let Some(cn) = doc.node(cid) else { continue };
        let props = variant_props(&cn.name);
        let Some(state_val) = matching_state_value(&base_props, &props) else { continue };
        match state_val.as_str() {
            "hover" | "hovered" | "mouseover" => hover = hover.or(Some(cid)),
            "active" | "pressed" | "down" | "mousedown" => active = active.or(Some(cid)),
            _ => {}
        }
    }
    Some((hover, active))
}

fn matching_state_value(base: &BTreeMap<String, String>, cand: &BTreeMap<String, String>) -> Option<String> {
    let mut state_val = None;
    for (k, v) in cand {
        let kl = k.to_ascii_lowercase();
        let is_state_key = matches!(kl.as_str(), "state" | "interaction" | "status" | "variant");
        match base.get(k) {
            Some(bv) if bv == v => {}
            _ if is_state_key && state_val.is_none() => state_val = Some(v.trim().to_ascii_lowercase()),
            _ => return None,
        }
    }
    state_val
}

fn state_visual_decls(ctx: &mut EmitCtx<'_>, target: &Node) -> Vec<(String, String)> {
    let mut decls = Vec::new();
    append_visual_decls(ctx, target, false, &mut decls);
    // If the state variant changes the primary label color inside a button/component, capture `color` too.
    if !matches!(target.kind, NodeKind::Text(_))
        && let Some(text_child) = target.children.iter().filter_map(|c| ctx.doc.node(*c)).find(|c| c.visible && matches!(c.kind, NodeKind::Text(_)))
        && let Some(fill) = text_child.fills.iter().find(|f| f.visible)
        && let PaintKind::Solid { color } = &fill.kind
    {
        decls.push(("color".into(), color.with_alpha(color.a * fill.opacity).to_css()));
    }
    decls
}

fn diff_state_decls(base: &[(String, String)], target: &[(String, String)]) -> Vec<(String, String)> {
    let base_map: BTreeMap<&str, &str> = base.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut out = Vec::new();
    for (k, v) in target {
        if matches!(k.as_str(), "cursor" | "margin" | "transition") {
            continue;
        }
        if base_map.get(k.as_str()).copied() != Some(v.as_str()) {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

fn interaction_transition_css(n: &Node) -> Option<String> {
    let it = n.interactions.iter().find(|i| matches!(i.trigger, Trigger::OnHover | Trigger::MouseEnter | Trigger::OnPress | Trigger::MouseDown))?;
    let ms = it.duration_ms.clamp(60, 2000);
    let easing = match it.easing {
        Easing::Linear => "linear".to_string(),
        Easing::EaseIn => "ease-in".to_string(),
        Easing::EaseOut => "ease-out".to_string(),
        Easing::EaseInOut => "ease-in-out".to_string(),
        Easing::CustomBezier { x1, y1, x2, y2 } => format!("cubic-bezier({}, {}, {}, {})", num(x1), num(y1), num(x2), num(y2)),
        _ => "ease-out".to_string(),
    };
    Some(format!("all {ms}ms {easing}"))
}

// ------------------------------------------------------------------ Variables & Responsive `@media`

fn build_variables_css(doc: &Document, names: &CssNames, bound_fallbacks: &[(String, String)], include_all: bool) -> (String, usize) {
    let mut root_vars: Vec<(String, String)> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut mode_blocks = String::new();

    if include_all {
        let vars = &doc.variables;
        let flattener = vars.flattener();
        let roles = crate::token_roles::Roles::new(doc);
        for c in local_collections(doc) {
            let attr = crate::codegen::css_var_name(&c.name);
            let attr_key = attr.trim_start_matches('-');
            for (mode_idx, mode) in c.modes.iter().enumerate() {
                let mut entries = Vec::new();
                for v in vars.variables.iter().filter(|v| v.collection == c.id) {
                    let Some(val_str) = resolve_variable_css(vars, &flattener, &roles, names, v, &mode.id) else {
                        continue;
                    };
                    let var_name = names.name(v);
                    if mode_idx == 0 {
                        if seen.insert(var_name.clone()) {
                            root_vars.push((var_name, val_str));
                        }
                    } else {
                        entries.push((var_name, val_str));
                    }
                }
                if mode_idx > 0 && !entries.is_empty() {
                    let safe_mode = mode.name.replace(['"', '\\', '\n', '\r'], " ");
                    mode_blocks.push_str(&format_rule(&format!("[data-{attr_key}=\"{safe_mode}\"]"), &entries, ""));
                    mode_blocks.push('\n');
                    if mode.name.trim().eq_ignore_ascii_case("dark") {
                        mode_blocks.push_str("@media (prefers-color-scheme: dark) {\n");
                        mode_blocks.push_str(&format_rule(&format!(":root:not([data-{attr_key}])"), &entries, "  "));
                        mode_blocks.push_str("}\n\n");
                    }
                }
            }
        }
    }

    for (name, fb) in bound_fallbacks {
        if seen.insert(name.clone()) {
            root_vars.push((name.clone(), fb.clone()));
        }
    }

    if root_vars.is_empty() && mode_blocks.is_empty() {
        return (String::new(), 0);
    }
    let mut out = String::new();
    if !root_vars.is_empty() {
        out.push_str(&format_rule(":root", &root_vars, ""));
    }
    if !mode_blocks.is_empty() {
        out.push('\n');
        out.push_str(mode_blocks.trim_end());
        out.push('\n');
    }
    (out, root_vars.len())
}

fn resolve_variable_css(
    vars: &figcraft_doc::Variables,
    flattener: &figcraft_doc::Flattener<'_>,
    roles: &crate::token_roles::Roles,
    names: &CssNames,
    v: &Variable,
    mode_id: &figcraft_doc::ModeId,
) -> Option<String> {
    let raw = v.values.get(mode_id)?;
    let flat = flattener.value(v, mode_id, raw)?;
    match flat {
        VariableValue::Color(col) => Some(col.to_css()),
        VariableValue::Number(x) if roles.role(v) == crate::token_roles::NumberRole::Unitless => Some(trim_num(x)),
        VariableValue::Number(x) => Some(format!("{}px", trim_num(x))),
        VariableValue::String(s) => Some(format!("\"{}\"", s.replace(['"', '\\', '\n', '\r'], " "))),
        VariableValue::Boolean(b) => Some(b.to_string()),
        VariableValue::Alias(a) => vars.get_by_id(&a).map(|target| format!("var({})", names.name(target))),
        VariableValue::ColorAlpha { .. } | VariableValue::ColorMix { .. } => None,
    }
}

fn build_media_queries(doc: &Document, ctx: &mut EmitCtx<'_>, variant_frames: &[(u32, String, NodeId)], tiers: &[BreakpointTier]) -> String {
    if tiers.is_empty() && variant_frames.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut covered_widths: BTreeSet<u32> = BTreeSet::new();

    // 1. Exact multi-frame artboard diffs (sorted widest -> narrowest, e.g. Tablet 768px then Mobile 375px).
    let mut sorted_variants = variant_frames.to_vec();
    sorted_variants.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (width, label, frame_id) in sorted_variants {
        let block = diff_variant_frame(doc, ctx, frame_id);
        if !block.is_empty() {
            covered_widths.insert(width);
            out.push_str(&format!("/* {label} ({width}px) */\n@media (max-width: {width}px) {{\n{block}}}\n\n"));
        }
    }

    // 2. Synthesized responsive rules for standard breakpoints (`Desktop 1440px`, `Tablet 768px`, `Mobile 375px`)
    // not already covered by a dedicated variant frame.
    let mut desc_tiers = tiers.to_vec();
    desc_tiers.sort_by_key(|x| std::cmp::Reverse(x.width));
    for tier in desc_tiers {
        if covered_widths.iter().any(|w| w.abs_diff(tier.width) <= 48) {
            continue;
        }
        let synth = synthesize_tier_rules(&ctx.path_rules, tier.width);
        if !synth.is_empty() {
            if tier.width >= 1200 {
                out.push_str(&format!("/* {} ({}px) */\n@media (min-width: {}px) {{\n{synth}}}\n\n", tier.name, tier.width, tier.width));
            } else {
                out.push_str(&format!("/* {} ({}px) */\n@media (max-width: {}px) {{\n{synth}}}\n\n", tier.name, tier.width, tier.width));
            }
        }
    }

    out
}

fn diff_variant_frame(doc: &Document, ctx: &mut EmitCtx<'_>, frame_id: NodeId) -> String {
    let mut var_ctx = EmitCtx {
        doc,
        var_names: CssNames::new(doc),
        used_classes: Names::default(),
        bound_vars: Vec::new(),
        css_blocks: Vec::new(),
        state_blocks: Vec::new(),
        path_rules: BTreeMap::new(),
        images: BTreeMap::new(),
    };
    let mut scratch_html = String::new();
    emit_node(&mut var_ctx, frame_id, None, 0, "", true, &mut scratch_html);

    let mut rules = String::new();
    for (path, (base_cls, base_decls, _)) in &ctx.path_rules {
        if let Some((_, var_decls, _)) = var_ctx.path_rules.get(path) {
            let diff = diff_state_decls(base_decls, var_decls);
            if !diff.is_empty() {
                rules.push_str(&format_rule(&format!(".{base_cls}"), &diff, "  "));
            }
        } else if !path.is_empty() {
            // Layer exists in wider frame but was omitted in narrower breakpoint frame.
            rules.push_str(&format_rule(&format!(".{base_cls}"), &[("display".into(), "none".into())], "  "));
        }
    }
    rules
}

fn synthesize_tier_rules(path_rules: &PathRules, width: u32) -> String {
    let mut out = String::new();
    for (cls, _, meta) in path_rules.values() {
        let mut overrides = Vec::new();
        if width >= 1200 {
            if meta.is_root {
                overrides.push(("max-width".into(), format!("{width}px")));
            }
        } else if width >= 600 {
            // Tablet (e.g. 768px)
            if meta.is_root {
                overrides.push(("max-width".into(), format!("{width}px")));
            }
            if meta.grid_cols > 2 {
                overrides.push(("grid-template-columns".into(), "repeat(2, minmax(0, 1fr))".into()));
            } else if meta.is_flex_row && meta.child_count >= 3 && meta.width > f64::from(width) {
                overrides.push(("flex-wrap".into(), "wrap".into()));
            }
            if meta.padding_h > 32.0 {
                overrides.push(("padding-left".into(), "24px".into()));
                overrides.push(("padding-right".into(), "24px".into()));
            }
            if let Some(fs) = meta.font_size.filter(|s| *s >= 44.0) {
                overrides.push(("font-size".into(), format!("{}px", num((fs * 0.85).round()))));
            }
        } else {
            // Mobile (e.g. 375px)
            if meta.is_root {
                overrides.push(("max-width".into(), format!("{width}px")));
            }
            if meta.grid_cols > 1 {
                overrides.push(("grid-template-columns".into(), "minmax(0, 1fr)".into()));
            } else if meta.is_flex_row && meta.child_count >= 2 && meta.width >= 320.0 {
                overrides.push(("flex-direction".into(), "column".into()));
                overrides.push(("align-items".into(), "stretch".into()));
            }
            if meta.padding_h > 20.0 {
                overrides.push(("padding-left".into(), "16px".into()));
                overrides.push(("padding-right".into(), "16px".into()));
            }
            if let Some(fs) = meta.font_size.filter(|s| *s >= 32.0) {
                overrides.push(("font-size".into(), format!("{}px", num((fs * 0.75).max(24.0).round()))));
            }
        }
        if !overrides.is_empty() {
            out.push_str(&format_rule(&format!(".{cls}"), &overrides, "  "));
        }
    }
    out
}

// ------------------------------------------------------------------ Formatting helpers

fn format_rule(selector: &str, decls: &[(String, String)], indent: &str) -> String {
    let mut out = format!("{indent}{selector} {{\n");
    for (k, v) in decls {
        out.push_str(&format!("{indent}  {k}: {v};\n"));
    }
    out.push_str(&format!("{indent}}}\n"));
    out
}

fn inline_html_doc(title: &str, body: &str, css: &str) -> String {
    let t = escape_html(title);
    let indented_body = indent_lines(body, "  ");
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n  <title>{t}</title>\n  <style>\n{css}  </style>\n</head>\n<body>\n{indented_body}</body>\n</html>\n"
    )
}

fn external_html_doc(title: &str, body: &str, stylesheet_href: &str) -> String {
    let t = escape_html(title);
    let href = escape_attr(stylesheet_href);
    let indented_body = indent_lines(body, "  ");
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n  <title>{t}</title>\n  <link rel=\"stylesheet\" href=\"{href}\" />\n</head>\n<body>\n{indented_body}</body>\n</html>\n"
    )
}

fn indent_lines(text: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    for line in text.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str(prefix);
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape_html(s).replace('"', "&quot;")
}

#[allow(dead_code)]
fn _keep_engine_error_used(_: EngineError) {}

#[cfg(test)]
mod tests {
    use figcraft_doc::NodeKind;
    use serde_json::json;

    use crate::Session;
    use crate::params::id_of;

    fn make(s: &mut Session, cmd: &str, p: serde_json::Value) -> figcraft_doc::NodeId {
        id_of(&s.execute(cmd, &p).unwrap()["id"]).unwrap()
    }

    #[test]
    fn live_html_css_emits_semantic_tags_variables_grid_states_and_breakpoints() {
        let mut s = Session::new();

        // FigCraft Variables with Light and Dark modes.
        s.execute("variable.createCollection", &json!({"name": "Brand", "modes": ["Light", "Dark"]})).unwrap();
        s.execute("variable.create", &json!({"collection": "Brand", "name": "surface", "type": "color", "value": "#FFFFFF"})).unwrap();
        s.execute("variable.set", &json!({"variable": "surface", "mode": "Dark", "value": "#0F172A"})).unwrap();
        s.execute("variable.create", &json!({"collection": "Brand", "name": "accent", "type": "color", "value": "#2563EB"})).unwrap();

        // Desktop 1440px page frame with Header, Grid Feature section, and interactive CTA component variants.
        let desktop = make(
            &mut s,
            "frame.create",
            json!({"x": 0, "y": 0, "width": 1440, "height": 900, "name": "Landing - Desktop", "props": {"layout": "vertical", "gap": 32, "padding": 48}}),
        );
        s.execute("variable.bind", &json!({"ids": [desktop], "field": "fill", "variable": "surface"})).unwrap();

        let header = make(
            &mut s,
            "frame.create",
            json!({"parent": desktop, "x": 0, "y": 0, "width": 1344, "height": 72, "name": "Header", "props": {"layout": "horizontal", "gap": 24, "padding": 16}}),
        );
        let _title = make(
            &mut s,
            "text.create",
            json!({"parent": header, "x": 0, "y": 0, "text": "ArtCraft Studio", "name": "H1 Brand", "props": {"fontSize": 36}}),
        );

        let features = make(
            &mut s,
            "frame.create",
            json!({"parent": desktop, "x": 0, "y": 100, "width": 1344, "height": 320, "name": "Features Section", "props": {"layout": "grid", "gridColumns": 3, "gap": 24, "padding": 16}}),
        );
        for idx in 1..=3 {
            let card =
                make(&mut s, "frame.create", json!({"parent": features, "x": 0, "y": 0, "width": 360, "height": 200, "name": format!("Card {idx}")}));
            s.execute("text.create", &json!({"parent": card, "x": 16, "y": 16, "text": format!("Feature {idx}"), "props": {"fontSize": 22}}))
                .unwrap();
        }

        // Interactive button variants (`State=Default`, `State=Hover`, `State=Active`).
        let v_default = make(
            &mut s,
            "frame.create",
            json!({"x": 2000, "y": 0, "width": 180, "height": 48, "name": "State=Default", "props": {"fill": "#2563EB"}}),
        );
        s.execute("variable.bind", &json!({"ids": [v_default], "field": "fill", "variable": "accent"})).unwrap();
        let v_default = id_of(&s.execute("component.create", &json!({"ids": [v_default]})).unwrap()["id"]).unwrap();

        let v_hover =
            make(&mut s, "frame.create", json!({"x": 2220, "y": 0, "width": 180, "height": 48, "name": "State=Hover", "props": {"fill": "#1D4ED8"}}));
        let v_hover = id_of(&s.execute("component.create", &json!({"ids": [v_hover]})).unwrap()["id"]).unwrap();

        let v_active = make(
            &mut s,
            "frame.create",
            json!({"x": 2440, "y": 0, "width": 180, "height": 48, "name": "State=Active", "props": {"fill": "#1E40AF"}}),
        );
        let v_active = id_of(&s.execute("component.create", &json!({"ids": [v_active]})).unwrap()["id"]).unwrap();

        let set_id = id_of(&s.execute("component.combineVariants", &json!({"ids": [v_default, v_hover, v_active]})).unwrap()["id"]).unwrap();
        s.execute("node.set", &json!({"ids": [set_id], "name": "CTA Button"})).unwrap();
        let cta_inst =
            id_of(&s.execute("instance.create", &json!({"component": v_default, "parent": header, "x": 0, "y": 0})).unwrap()["id"]).unwrap();
        s.execute("node.set", &json!({"ids": [cta_inst], "name": "CTA Button"})).unwrap();

        let out = s.execute("code.liveHtmlCss", &json!({"ids": [desktop]})).unwrap();
        let html = out["html"].as_str().unwrap();
        let css = out["css"].as_str().unwrap();

        assert!(html.contains("<header "), "expected semantic <header>: {html}");
        assert!(html.contains("<h1 "), "expected semantic <h1>: {html}");
        assert!(html.contains("<section "), "expected semantic <section>: {html}");
        assert!(html.contains("<article "), "expected semantic <article> cards: {html}");
        assert!(html.contains("<button "), "expected semantic <button> CTA: {html}");

        assert!(css.contains(":root {"), "expected :root custom properties: {css}");
        assert!(css.contains("--surface: #FFFFFF;"), "expected Light variable: {css}");
        assert!(css.contains("prefers-color-scheme: dark"), "expected Dark mode media block: {css}");
        assert!(css.contains("display: grid;"), "expected CSS Grid layout: {css}");
        assert!(css.contains("repeat(3, minmax(0, 1fr))"), "expected 3-col grid template: {css}");
        assert!(css.contains(":hover {"), "expected :hover rule from State=Hover variant: {css}");
        assert!(css.contains("#1D4ED8"), "expected hover fill override: {css}");
        assert!(css.contains(":active {"), "expected :active rule from State=Active variant: {css}");
        assert!(css.contains("#1E40AF"), "expected active fill override: {css}");
        assert!(css.contains("@media (max-width: 768px)"), "expected Tablet 768px media query: {css}");
        assert!(css.contains("@media (max-width: 375px)"), "expected Mobile 375px media query: {css}");
    }

    #[test]
    fn export_html_site_writes_bundle_and_import_html_reconstructs_editable_layers() {
        let mut s = Session::new();
        let snippet = r#"
<!doctype html>
<html>
<head>
  <title>Dreamweaver Card</title>
  <style>
    :root { --card-bg: #F8FAFC; --brand: #2563EB; }
    .hero { display: flex; flex-direction: column; gap: 20px; padding: 32px; background-color: var(--card-bg); border-radius: 16px; border: 1px solid #CBD5E1; width: 720px; }
    .grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 16px; width: 100%; }
    .card { padding: 16px; background: #FFFFFF; border-radius: 12px; }
    .cta { background: var(--brand); color: #FFFFFF; padding: 12px 24px; border-radius: 8px; font-weight: 700; }
  </style>
</head>
<body>
  <section class="hero" id="HeroSection">
    <h1 style="color: #0F172A; font-size: 36px;">Responsive Design</h1>
    <p style="color: #475569; font-size: 16px;">Semantic HTML5 &amp; CSS3 Flexbox/Grid authoring.</p>
    <div class="grid">
      <article class="card"><h3>Mobile 375px</h3></article>
      <article class="card"><h3>Desktop 1440px</h3></article>
    </div>
    <button class="cta">Launch Site</button>
  </section>
</body>
</html>
"#;

        let imported = s.execute("file.importHtml", &json!({"html": snippet, "x": 40, "y": 60})).unwrap();
        let root_id = id_of(&imported["id"]).unwrap();
        assert!(imported["layers"].as_u64().unwrap() >= 6);

        let doc = s.doc().unwrap();
        let root = doc.node(root_id).unwrap();
        assert_eq!(root.kind, NodeKind::Frame);
        assert!(root.layout.is_some());
        assert_eq!(root.corners.uniform(), Some(16.0));
        assert_eq!(root.strokes.len(), 1);

        // Verify text & grid children came through as editable FigCraft layers.
        let subtree = doc.subtree(root_id);
        let has_heading = subtree
            .iter()
            .filter_map(|id| doc.node(*id))
            .any(|n| matches!(&n.kind, NodeKind::Text(tx) if tx.characters == "Responsive Design" && (tx.style.font_size - 36.0).abs() < 1e-6));
        assert!(has_heading, "expected imported 36px heading text layer");
        let has_grid = subtree
            .iter()
            .filter_map(|id| doc.node(*id))
            .any(|n| n.layout.as_ref().is_some_and(|l| l.mode == figcraft_doc::LayoutMode::Grid && l.grid_columns == 2));
        assert!(has_grid, "expected imported 2-column CSS Grid AutoLayout frame");

        // Export to a temporary directory bundle (`index.html` + `styles.css`) and verify on disk.
        let tmp = std::env::temp_dir().join(format!("figcraft_dw_site_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let exported = s.execute("file.exportHtmlSite", &json!({"ids": [root_id], "dir": tmp.to_string_lossy()})).unwrap();
        let files = exported["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        let index_on_disk = std::fs::read_to_string(tmp.join("index.html")).unwrap();
        let css_on_disk = std::fs::read_to_string(tmp.join("styles.css")).unwrap();
        assert!(index_on_disk.contains("<link rel=\"stylesheet\" href=\"styles.css\" />"));
        assert!(index_on_disk.contains("Responsive Design"));
        assert!(css_on_disk.contains("display: grid;"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
