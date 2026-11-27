//! Clean-room HTML5 + CSS3 snippet & page importer (`file.importHtml`).
//!
//! Parses semantic HTML5 elements (`<header>`, `<nav>`, `<main>`, `<section>`, `<article>`,
//! `<aside>`, `<footer>`, `<div>`, `<button>`, `<a>`, `<h1>`–`<h6>`, `<p>`, `<ul>`/`<ol>`/`<li>`,
//! `<hr>`, `<img>`, etc.) together with `<style>` rules (`:root` custom properties, tag, `.class`,
//! `#id` and compound selectors) and inline `style="…"` declarations into editable FigCraft
//! `Frame`, `Text`, `Rectangle` and `Ellipse` layers.

use std::collections::BTreeMap;

use base64::Engine as _;
use figcraft_color::{Color, ColorStop, Gradient, GradientKind, Paint};
use figcraft_doc::{
    AutoLayout, Corners, CrossAlign, Document, Effect, EffectKind, GridTrack, LayoutMode, LetterSpacing, LineHeight, ListStyle, MainAlign, NodeId,
    NodeKind, Shadow, Sizing, StrokeProps, TextAlignH, TextAutoResize, TextCase, TextData, TextDecoration, TextStyle,
};
use figcraft_geom::Affine;
use serde_json::{Value, json};

use crate::params::{num, opt_id, string};
use crate::{Result, Session, bad};

/// Maximum HTML input size accepted by `file.importHtml` (8 MiB).
const MAX_HTML_BYTES: u64 = 8 << 20;
/// Maximum DOM elements constructed from one HTML document.
const MAX_DOM_NODES: usize = 2_000;
/// Maximum DOM nesting depth.
const MAX_DOM_DEPTH: usize = 24;

#[derive(Clone, Debug)]
enum HtmlNode {
    Element(HtmlElem),
    Text(String),
}

#[derive(Clone, Debug, Default)]
struct HtmlElem {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    attrs: BTreeMap<String, String>,
    children: Vec<HtmlNode>,
}

#[derive(Clone, Debug)]
struct CssRule {
    selector: String,
    decls: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
struct Stylesheet {
    vars: BTreeMap<String, String>,
    rules: Vec<CssRule>,
}

#[derive(Clone, Debug)]
struct InheritedText {
    family: String,
    size: f64,
    weight: u16,
    italic: bool,
    color: Color,
    align: TextAlignH,
    line_height: LineHeight,
    letter_spacing: LetterSpacing,
    list: ListStyle,
}

impl Default for InheritedText {
    fn default() -> Self {
        Self {
            family: figcraft_doc::DEFAULT_FONT.to_string(),
            size: 16.0,
            weight: 400,
            italic: false,
            color: Color::from_rgb8_const(0x11, 0x18, 0x27),
            align: TextAlignH::Left,
            line_height: LineHeight::Auto,
            letter_spacing: LetterSpacing::default(),
            list: ListStyle::None,
        }
    }
}

struct BuildState {
    created: Vec<NodeId>,
    warnings: Vec<String>,
    count: usize,
}

/// Execute `file.importHtml`.
pub fn import_html(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "file.importHtml";
    let (raw_html, fallback_name) = read_html_source(CMD, p)?;
    let page = s.page()?;
    let parent = opt_id(CMD, p, "parent")?.unwrap_or(page);
    if !s.doc()?.get(parent)?.kind.is_container() {
        return Err(bad(CMD, format!("{parent} can't contain layers")));
    }
    let center = s.ds().map_or((0.0, 0.0), |d| (d.view.cx, d.view.cy));
    let (x, y) = (num(p, "x").unwrap_or(center.0), num(p, "y").unwrap_or(center.1));
    let root_width = num(p, "width").filter(|w| w.is_finite() && *w >= 40.0 && *w <= 16_384.0).unwrap_or(1200.0);
    let requested_name = string(p, "name").filter(|n| !n.trim().is_empty());

    let doc = s.doc_mut()?;
    let (root_id, all_ids, warnings) = import_html_into_doc(doc, &raw_html, parent, x, y, root_width, requested_name.as_deref(), &fallback_name)?;
    s.set_selection(vec![root_id]);
    Ok(json!({
        "id": root_id,
        "ids": all_ids,
        "layers": all_ids.len(),
        "warnings": warnings,
    }))
}

fn read_html_source(cmd: &str, p: &Value) -> Result<(String, String)> {
    if let Some(h) = string(p, "html") {
        if h.len() as u64 > MAX_HTML_BYTES {
            return Err(bad(cmd, "HTML input exceeds 8 MB"));
        }
        return Ok((h, string(p, "name").unwrap_or_else(|| "HTML Snippet".into())));
    }
    if let Some(path) = string(p, "path") {
        let bytes = crate::file_access::read_capped(&path, MAX_HTML_BYTES)?;
        let stem = std::path::Path::new(&path).file_stem().and_then(std::ffi::OsStr::to_str).unwrap_or("Imported HTML").to_string();
        return Ok((String::from_utf8_lossy(&bytes).into_owned(), stem));
    }
    if let Some(d) = string(p, "data") {
        let payload = d.split_once(',').map_or(d.as_str(), |(_, b)| b);
        let bytes = base64::engine::general_purpose::STANDARD.decode(payload.trim()).map_err(|e| bad(cmd, format!("`data`: {e}")))?;
        if bytes.len() as u64 > MAX_HTML_BYTES {
            return Err(bad(cmd, "HTML input exceeds 8 MB"));
        }
        let stem = string(p, "name")
            .as_deref()
            .and_then(|n| std::path::Path::new(n).file_stem())
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("Imported HTML")
            .to_string();
        return Ok((String::from_utf8_lossy(&bytes).into_owned(), stem));
    }
    Err(bad(cmd, "pass `html`, `path` or `data`"))
}

/// Parse `html` and construct editable layers inside `parent` at `(x, y)`.
#[allow(clippy::too_many_arguments)]
pub fn import_html_into_doc(
    doc: &mut Document,
    html: &str,
    parent: NodeId,
    x: f64,
    y: f64,
    default_width: f64,
    explicit_name: Option<&str>,
    fallback_name: &str,
) -> Result<(NodeId, Vec<NodeId>, Vec<String>)> {
    let mut state = BuildState { created: Vec::new(), warnings: Vec::new(), count: 0 };
    let (dom, mut styles, title) = parse_html_document(html, &mut state.warnings);
    let body_nodes = extract_visual_roots(&dom, &styles);
    let inherited = InheritedText::default();

    let root_name = explicit_name.or(title.as_deref()).filter(|s| !s.trim().is_empty()).unwrap_or(fallback_name);

    // If there is exactly one visual root element and it is already a container, build it directly
    // as the root frame; otherwise wrap the snippet in a responsive root Frame.
    let root_id = match body_nodes.as_slice() {
        [HtmlNode::Element(el)] if !is_pure_text_element(el, &resolve_styles(el, &styles)) && !is_void_or_leaf_shape(el) => {
            let id = build_element(doc, parent, el, &mut styles, &inherited, true, default_width, &mut state)?;
            if let Some(n) = doc.node_mut(id) {
                if explicit_name.is_some() || n.name.eq_ignore_ascii_case("body") {
                    n.name = root_name.to_string();
                }
                n.transform = Affine::translate((x, y));
            }
            id
        }
        _ => {
            let wrapper = doc.create(parent, NodeKind::Frame, root_name, x, y, default_width, 100.0)?;
            state.created.push(wrapper);
            if let Some(w) = doc.node_mut(wrapper) {
                w.fills = vec![Paint::solid(Color::WHITE)];
                w.layout = Some(AutoLayout { mode: LayoutMode::Vertical, gap: 16.0, cross_gap: 16.0, padding: [24.0; 4], ..AutoLayout::default() });
                w.child_layout.sizing_h = Sizing::Fixed;
                w.child_layout.sizing_v = Sizing::Hug;
            }
            for child in &body_nodes {
                build_html_node(doc, wrapper, child, &mut styles, &inherited, default_width - 48.0, &mut state)?;
            }
            wrapper
        }
    };

    Ok((root_id, state.created, state.warnings))
}

fn extract_visual_roots(nodes: &[HtmlNode], styles: &Stylesheet) -> Vec<HtmlNode> {
    let unwrap_body = |b: &HtmlElem| -> Vec<HtmlNode> {
        let visual_children: Vec<HtmlNode> = b.children.iter().filter(|c| !is_non_visual(c)).cloned().collect();
        let body_props = resolve_styles(b, styles);
        if b.id.is_none() && b.classes.is_empty() && body_props.is_empty() && !visual_children.is_empty() {
            visual_children
        } else {
            vec![HtmlNode::Element(b.clone())]
        }
    };
    for n in nodes {
        if let HtmlNode::Element(el) = n
            && el.tag == "html"
        {
            for c in &el.children {
                if let HtmlNode::Element(b) = c
                    && b.tag == "body"
                {
                    return unwrap_body(b);
                }
            }
            return el.children.iter().filter(|c| !is_non_visual(c)).cloned().collect();
        }
        if let HtmlNode::Element(el) = n
            && el.tag == "body"
        {
            return unwrap_body(el);
        }
    }
    nodes.iter().filter(|n| !is_non_visual(n)).cloned().collect()
}

fn is_non_visual(node: &HtmlNode) -> bool {
    match node {
        HtmlNode::Text(t) => t.trim().is_empty(),
        HtmlNode::Element(el) => {
            matches!(el.tag.as_str(), "head" | "title" | "meta" | "link" | "style" | "script" | "noscript" | "template")
        }
    }
}

fn build_html_node(
    doc: &mut Document,
    parent: NodeId,
    node: &HtmlNode,
    styles: &mut Stylesheet,
    inherited: &InheritedText,
    avail_w: f64,
    state: &mut BuildState,
) -> Result<Option<NodeId>> {
    if state.count >= MAX_DOM_NODES {
        if state.warnings.len() < 8 {
            state.warnings.push(format!("HTML node limit ({MAX_DOM_NODES}) reached; remaining elements skipped"));
        }
        return Ok(None);
    }
    match node {
        HtmlNode::Text(text) => {
            let cleaned = normalize_ws(text);
            if cleaned.is_empty() {
                return Ok(None);
            }
            state.count += 1;
            let id = create_text_layer(doc, parent, "Text", &cleaned, inherited, &BTreeMap::new(), avail_w)?;
            state.created.push(id);
            Ok(Some(id))
        }
        HtmlNode::Element(el) => {
            if is_non_visual(node) {
                return Ok(None);
            }
            let id = build_element(doc, parent, el, styles, inherited, false, avail_w, state)?;
            Ok(Some(id))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_element(
    doc: &mut Document,
    parent: NodeId,
    el: &HtmlElem,
    styles: &mut Stylesheet,
    inherited: &InheritedText,
    is_root: bool,
    avail_w: f64,
    state: &mut BuildState,
) -> Result<NodeId> {
    state.count += 1;
    let prop_map = resolve_styles(el, styles);
    if prop_map.get("display").is_some_and(|v| v.eq_ignore_ascii_case("none")) {
        let hidden = doc.create(parent, NodeKind::Frame, &element_label(el), 0.0, 0.0, 10.0, 10.0)?;
        if let Some(n) = doc.node_mut(hidden) {
            n.visible = false;
        }
        state.created.push(hidden);
        return Ok(hidden);
    }

    let next_text = derive_text_style(&el.tag, inherited, &prop_map);

    if is_pure_text_element(el, &prop_map) {
        let content = collect_inline_text(&el.children);
        let label = element_label(el);
        let id = create_text_layer(doc, parent, &label, &content, &next_text, &prop_map, avail_w)?;
        if let Some(href) = el.attrs.get("href").filter(|h| !h.is_empty())
            && let Some(NodeKind::Text(tx)) = doc.node_mut(id).map(|n| &mut n.kind)
        {
            tx.style.link = Some(href.clone());
        }
        apply_child_placement(doc, id, parent, &prop_map, is_root);
        state.created.push(id);
        return Ok(id);
    }

    if is_void_or_leaf_shape(el) {
        let id = create_shape_layer(doc, parent, el, &prop_map, avail_w)?;
        apply_child_placement(doc, id, parent, &prop_map, is_root);
        state.created.push(id);
        return Ok(id);
    }

    build_container_frame(doc, parent, el, styles, &next_text, &prop_map, is_root, avail_w, state)
}

#[allow(clippy::too_many_arguments)]
fn build_container_frame(
    doc: &mut Document,
    parent: NodeId,
    el: &HtmlElem,
    styles: &mut Stylesheet,
    text_ctx: &InheritedText,
    prop_map: &BTreeMap<String, String>,
    is_root: bool,
    avail_w: f64,
    state: &mut BuildState,
) -> Result<NodeId> {
    let explicit_w = prop_map
        .get("width")
        .and_then(|v| parse_px_len(v, text_ctx.size))
        .or_else(|| prop_map.get("max-width").and_then(|v| parse_px_len(v, text_ctx.size)));
    let w = explicit_w.unwrap_or_else(|| if is_root { avail_w } else { avail_w.max(120.0) }).clamp(1.0, 16_384.0);
    let explicit_h = prop_map
        .get("height")
        .and_then(|v| parse_px_len(v, text_ctx.size))
        .or_else(|| prop_map.get("min-height").and_then(|v| parse_px_len(v, text_ctx.size)));
    let h = explicit_h.unwrap_or(64.0).clamp(1.0, 16_384.0);

    let id = doc.create(parent, NodeKind::Frame, &element_label(el), 0.0, 0.0, w, h)?;
    state.created.push(id);

    let abs_children = el.children.iter().any(|c| match c {
        HtmlNode::Element(ce) => resolve_styles(ce, styles).get("position").is_some_and(|v| v.eq_ignore_ascii_case("absolute")),
        HtmlNode::Text(_) => false,
    });

    let pad = parse_box_sides(prop_map, "padding", text_ctx.size, default_padding_for_tag(&el.tag));
    let layout =
        if abs_children && !prop_map.contains_key("display") { None } else { Some(parse_container_layout(&el.tag, prop_map, pad, text_ctx.size)) };

    if let Some(n) = doc.node_mut(id) {
        n.fills = parse_fills(prop_map, is_root && el.tag == "body");
        apply_borders_and_corners(n, prop_map, text_ctx.size);
        apply_effects_and_opacity(n, prop_map, text_ctx.size);
        n.clip = prop_map.get("overflow").is_some_and(|v| v.contains("hidden"));
        n.layout = layout;
        if is_root {
            n.child_layout.sizing_h = Sizing::Fixed;
            n.child_layout.sizing_v = if prop_map.contains_key("height") { Sizing::Fixed } else { Sizing::Hug };
        } else {
            n.child_layout.sizing_h = infer_h_sizing(&el.tag, prop_map, explicit_w.is_some());
            n.child_layout.sizing_v = if prop_map.contains_key("height") { Sizing::Fixed } else { Sizing::Hug };
        }
        n.child_layout.min_w = prop_map.get("min-width").and_then(|v| parse_px_len(v, text_ctx.size));
        n.child_layout.max_w = prop_map.get("max-width").and_then(|v| parse_px_len(v, text_ctx.size));
        n.child_layout.min_h = prop_map.get("min-height").and_then(|v| parse_px_len(v, text_ctx.size));
        n.child_layout.max_h = prop_map.get("max-height").and_then(|v| parse_px_len(v, text_ctx.size));
    }
    apply_child_placement(doc, id, parent, prop_map, is_root);

    let inner_w = (w - pad[1] - pad[3]).max(40.0);
    // Styled buttons / links whose children are inline text get a single crisp Text child.
    if matches!(el.tag.as_str(), "button" | "a") && el.children.iter().all(is_inline_node) {
        let label = collect_inline_text(&el.children);
        if !label.is_empty() {
            let tid = create_text_layer(doc, id, "Label", &label, text_ctx, prop_map, inner_w)?;
            if let Some(href) = el.attrs.get("href").filter(|h| !h.is_empty())
                && let Some(NodeKind::Text(tx)) = doc.node_mut(tid).map(|n| &mut n.kind)
            {
                tx.style.link = Some(href.clone());
            }
            state.created.push(tid);
        }
        return Ok(id);
    }

    for child in &el.children {
        build_html_node(doc, id, child, styles, text_ctx, inner_w, state)?;
    }
    Ok(id)
}

fn is_inline_node(node: &HtmlNode) -> bool {
    match node {
        HtmlNode::Text(_) => true,
        HtmlNode::Element(e) => matches!(e.tag.as_str(), "span" | "strong" | "b" | "em" | "i" | "u" | "small" | "code" | "br"),
    }
}

fn is_pure_text_element(el: &HtmlElem, props: &BTreeMap<String, String>) -> bool {
    let has_box =
        props.contains_key("background") || props.contains_key("background-color") || props.contains_key("border") || props.contains_key("padding");
    let text_tag = matches!(
        el.tag.as_str(),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "span" | "label" | "strong" | "b" | "em" | "i" | "u" | "small" | "code" | "blockquote" | "li"
    );
    if matches!(el.tag.as_str(), "button" | "a") {
        return !has_box && el.children.iter().all(is_inline_node);
    }
    text_tag && !has_box && el.children.iter().all(is_inline_node)
}

fn is_void_or_leaf_shape(el: &HtmlElem) -> bool {
    if matches!(el.tag.as_str(), "hr" | "img" | "svg") {
        return true;
    }
    el.children.is_empty() && !matches!(el.tag.as_str(), "header" | "nav" | "main" | "section" | "article" | "aside" | "footer")
}

fn create_shape_layer(doc: &mut Document, parent: NodeId, el: &HtmlElem, props: &BTreeMap<String, String>, avail_w: f64) -> Result<NodeId> {
    let default_w = if el.tag == "hr" { avail_w } else { 160.0 };
    let default_h = if el.tag == "hr" { 1.0 } else { 100.0 };
    let w = el
        .attrs
        .get("width")
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| props.get("width").and_then(|v| parse_px_len(v, 16.0)))
        .unwrap_or(default_w)
        .clamp(1.0, 16_384.0);
    let h = el
        .attrs
        .get("height")
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| props.get("height").and_then(|v| parse_px_len(v, 16.0)))
        .unwrap_or(default_h)
        .clamp(1.0, 16_384.0);

    let circle = props.get("border-radius").is_some_and(|r| r.trim() == "50%" || r.trim() == "9999px") && (w - h).abs() < 1.0;
    let kind = if circle { NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 } } else { NodeKind::Rectangle };
    let id = doc.create(parent, kind, &element_label(el), 0.0, 0.0, w, h)?;
    if let Some(n) = doc.node_mut(id) {
        let mut fills = parse_fills(props, false);
        if fills.is_empty() {
            let fallback = if el.tag == "hr" { Color::from_rgb8_const(0xE5, 0xE7, 0xEB) } else { Color::from_rgb8_const(0xE2, 0xE8, 0xF0) };
            fills.push(Paint::solid(fallback));
        }
        n.fills = fills;
        if !circle {
            apply_borders_and_corners(n, props, 16.0);
        }
        apply_effects_and_opacity(n, props, 16.0);
        if el.tag == "hr" || props.get("width").is_some_and(|v| v.trim() == "100%") {
            n.child_layout.sizing_h = Sizing::Fill;
        }
    }
    Ok(id)
}

fn create_text_layer(
    doc: &mut Document,
    parent: NodeId,
    label: &str,
    content: &str,
    inherited: &InheritedText,
    props: &BTreeMap<String, String>,
    avail_w: f64,
) -> Result<NodeId> {
    let text = if content.is_empty() { " " } else { content };
    let style = TextStyle {
        font_family: inherited.family.clone(),
        font_style: font_style_name(inherited.weight, inherited.italic),
        font_size: inherited.size.clamp(4.0, 512.0),
        line_height: inherited.line_height,
        letter_spacing: inherited.letter_spacing,
        decoration: parse_text_decoration(props),
        case: parse_text_case(props),
        ..TextStyle::default()
    };
    let explicit_w = props.get("width").and_then(|v| parse_px_len(v, inherited.size));
    let fills_row = props.get("width").is_some_and(|v| v.trim() == "100%");
    let auto_resize =
        if explicit_w.is_some() || fills_row || content.chars().count() > 60 { TextAutoResize::Height } else { TextAutoResize::WidthAndHeight };
    let box_w = explicit_w.unwrap_or_else(|| avail_w.clamp(80.0, 720.0));
    let td = TextData { characters: text.to_string(), style, auto_resize, align_h: inherited.align, list: inherited.list, ..TextData::default() };
    let id = doc.create(parent, NodeKind::Text(td), label, 0.0, 0.0, box_w, (inherited.size * 1.4).max(16.0))?;
    if let Some(n) = doc.node_mut(id) {
        n.fills = vec![Paint::solid(inherited.color)];
        apply_effects_and_opacity(n, props, inherited.size);
        n.child_layout.sizing_h = if fills_row {
            Sizing::Fill
        } else if explicit_w.is_some() {
            Sizing::Fixed
        } else {
            Sizing::Hug
        };
        n.child_layout.sizing_v = Sizing::Hug;
    }
    Ok(id)
}

fn apply_child_placement(doc: &mut Document, id: NodeId, parent: NodeId, props: &BTreeMap<String, String>, is_root: bool) {
    if is_root {
        return;
    }
    let abs = props.get("position").is_some_and(|v| v.eq_ignore_ascii_case("absolute"));
    let parent_has_layout = doc.node(parent).is_some_and(|p| p.layout.is_some());
    if abs || !parent_has_layout {
        let left = props.get("left").and_then(|v| parse_px_len(v, 16.0)).unwrap_or(0.0);
        let top = props.get("top").and_then(|v| parse_px_len(v, 16.0)).unwrap_or(0.0);
        if let Some(n) = doc.node_mut(id) {
            if parent_has_layout {
                n.child_layout.absolute = true;
            }
            n.transform = Affine::translate((left, top));
        }
    }
    if props.get("flex").is_some_and(|v| v.trim().starts_with('1'))
        && let Some(n) = doc.node_mut(id)
    {
        n.child_layout.sizing_h = Sizing::Fill;
    }
}

fn infer_h_sizing(tag: &str, props: &BTreeMap<String, String>, has_explicit_w: bool) -> Sizing {
    if props.get("width").is_some_and(|v| v.trim() == "100%") || props.get("flex").is_some_and(|v| v.trim().starts_with('1')) {
        Sizing::Fill
    } else if has_explicit_w {
        Sizing::Fixed
    } else if matches!(tag, "header" | "nav" | "main" | "section" | "footer") {
        Sizing::Fill
    } else {
        Sizing::Hug
    }
}

fn default_padding_for_tag(tag: &str) -> [f64; 4] {
    match tag {
        "button" => [10.0, 18.0, 10.0, 18.0],
        _ => [0.0; 4],
    }
}

fn parse_container_layout(tag: &str, props: &BTreeMap<String, String>, padding: [f64; 4], em: f64) -> AutoLayout {
    let display = props.get("display").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    let (gap, cross_gap) = parse_gaps(props, em, if matches!(tag, "ul" | "ol") { 6.0 } else { 12.0 });

    if display.contains("grid") {
        let (cols, tracks) = parse_grid_columns(props.get("grid-template-columns").map(String::as_str), em);
        return AutoLayout {
            mode: LayoutMode::Grid,
            gap,
            cross_gap,
            padding,
            grid_columns: cols,
            grid_rows: 1,
            column_tracks: tracks,
            ..AutoLayout::default()
        };
    }

    let is_flex = display.contains("flex");
    let mode = if is_flex {
        let dir = props.get("flex-direction").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_else(|| "row".into());
        if dir.starts_with("column") { LayoutMode::Vertical } else { LayoutMode::Horizontal }
    } else if matches!(tag, "nav" | "button") && !display.contains("block") {
        LayoutMode::Horizontal
    } else {
        LayoutMode::Vertical
    };
    let wrap = props.get("flex-wrap").is_some_and(|v| v.to_ascii_lowercase().contains("wrap"));
    let main_align = match props.get("justify-content").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("center") => MainAlign::Center,
        Some("flex-end" | "end" | "right") => MainAlign::Max,
        Some("space-between") => MainAlign::SpaceBetween,
        Some("space-around") => MainAlign::SpaceAround,
        Some("space-evenly") => MainAlign::SpaceEvenly,
        _ if tag == "button" => MainAlign::Center,
        _ => MainAlign::Min,
    };
    let cross_align = match props.get("align-items").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("center") => CrossAlign::Center,
        Some("flex-end" | "end") => CrossAlign::Max,
        Some("baseline") => CrossAlign::Baseline,
        _ if matches!(tag, "button" | "nav") => CrossAlign::Center,
        _ => CrossAlign::Min,
    };
    AutoLayout { mode, wrap, gap, cross_gap, padding, main_align, cross_align, ..AutoLayout::default() }
}

fn parse_grid_columns(spec: Option<&str>, em: f64) -> (u32, Vec<GridTrack>) {
    let Some(s) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return (2, vec![GridTrack::fill(1.0), GridTrack::fill(1.0)]);
    };
    if let Some(inner) = s.strip_prefix("repeat(").and_then(|r| r.strip_suffix(')'))
        && let Some((count_s, track_s)) = inner.split_once(',')
        && let Ok(n) = count_s.trim().parse::<u32>()
    {
        let cols = n.clamp(1, 24);
        let t = parse_one_grid_track(track_s.trim(), em);
        return (cols, vec![t; cols as usize]);
    }
    let tracks: Vec<GridTrack> = split_top_level_ws(s).into_iter().take(24).map(|tok| parse_one_grid_track(tok, em)).collect();
    let cols = (tracks.len() as u32).max(1);
    (cols, tracks)
}

fn parse_one_grid_track(tok: &str, em: f64) -> GridTrack {
    let t = tok.trim();
    if let Some(fr) = t.strip_suffix("fr").and_then(|v| v.trim().parse::<f64>().ok()) {
        GridTrack::fill(fr.clamp(0.1, 24.0))
    } else if t.starts_with("minmax(") {
        GridTrack::fill(1.0)
    } else if t.eq_ignore_ascii_case("auto") {
        GridTrack::hug()
    } else if let Some(px) = parse_px_len(t, em) {
        GridTrack::fixed(px.max(1.0))
    } else {
        GridTrack::fill(1.0)
    }
}

fn parse_gaps(props: &BTreeMap<String, String>, em: f64, fallback: f64) -> (f64, f64) {
    let mut main_gap = if props.contains_key("display") { 0.0 } else { fallback };
    let mut cross_gap = main_gap;
    if let Some(g) = props.get("gap") {
        let parts: Vec<f64> = g.split_whitespace().filter_map(|t| parse_px_len(t, em)).collect();
        if let Some(&first) = parts.first() {
            main_gap = first;
            cross_gap = parts.get(1).copied().unwrap_or(first);
        }
    }
    if let Some(cg) = props.get("column-gap").and_then(|v| parse_px_len(v, em)) {
        main_gap = cg;
    }
    if let Some(rg) = props.get("row-gap").and_then(|v| parse_px_len(v, em)) {
        cross_gap = rg;
    }
    (main_gap.max(0.0), cross_gap.max(0.0))
}

fn parse_box_sides(props: &BTreeMap<String, String>, base: &str, em: f64, default: [f64; 4]) -> [f64; 4] {
    let mut sides = default;
    if let Some(shorthand) = props.get(base) {
        let vals: Vec<f64> = shorthand.split_whitespace().filter_map(|t| parse_px_len(t, em)).collect();
        sides = match vals.as_slice() {
            [a] => [*a, *a, *a, *a],
            [v, h] => [*v, *h, *v, *h],
            [t, h, b] => [*t, *h, *b, *h],
            [t, r, b, l, ..] => [*t, *r, *b, *l],
            [] => sides,
        };
    }
    for (idx, side) in ["top", "right", "bottom", "left"].iter().enumerate() {
        if let Some(v) = props.get(&format!("{base}-{side}")).and_then(|s| parse_px_len(s, em)) {
            sides[idx] = v.max(0.0);
        }
    }
    sides
}

fn parse_fills(props: &BTreeMap<String, String>, default_white: bool) -> Vec<Paint> {
    if let Some(bg) = props.get("background").or_else(|| props.get("background-image")).or_else(|| props.get("background-color")) {
        let b = bg.trim();
        if b.eq_ignore_ascii_case("none") || b.eq_ignore_ascii_case("transparent") {
            return Vec::new();
        }
        if let Some(grad) = parse_linear_gradient(b) {
            return vec![Paint::gradient(grad)];
        }
        if let Some(c) = Color::parse_css(b) {
            return if c.a <= 0.001 { Vec::new() } else { vec![Paint::solid(c)] };
        }
        // Shorthand `background: #f8fafc no-repeat`: pick the first CSS color token.
        for tok in split_top_level_ws(b) {
            if let Some(c) = Color::parse_css(tok) {
                return if c.a <= 0.001 { Vec::new() } else { vec![Paint::solid(c)] };
            }
        }
    }
    if default_white { vec![Paint::solid(Color::WHITE)] } else { Vec::new() }
}

fn parse_linear_gradient(s: &str) -> Option<Gradient> {
    let inner = s.strip_prefix("linear-gradient(").and_then(|r| r.strip_suffix(')'))?;
    let args = split_top_level_commas(inner);
    if args.len() < 2 {
        return None;
    }
    let first = args[0].trim();
    let (angle_deg, stop_slice) = if let Some(deg) = first.strip_suffix("deg").and_then(|v| v.trim().parse::<f64>().ok()) {
        (deg, &args[1..])
    } else if first.starts_with("to ") {
        let deg = match first {
            "to right" => 90.0,
            "to left" => 270.0,
            "to top" => 0.0,
            _ => 180.0,
        };
        (deg, &args[1..])
    } else {
        (180.0, args.as_slice())
    };

    let mut stops = Vec::new();
    let count = stop_slice.len().max(2);
    for (idx, item) in stop_slice.iter().enumerate() {
        let parts = split_top_level_ws(item.trim());
        let color_str = parts.first().copied()?;
        let color = Color::parse_css(color_str)?;
        let pos = parts
            .get(1)
            .and_then(|p| p.strip_suffix('%'))
            .and_then(|p| p.parse::<f64>().ok())
            .map_or_else(|| idx as f64 / (count - 1) as f64, |p| (p / 100.0).clamp(0.0, 1.0));
        stops.push(ColorStop { position: pos as f32, color });
    }
    if stops.len() < 2 {
        return None;
    }
    let rad = angle_deg.to_radians();
    let (dx, dy) = (rad.sin() * 0.5, -rad.cos() * 0.5);
    Some(Gradient { kind: GradientKind::Linear, stops, from: (0.5 - dx, 0.5 - dy), to: (0.5 + dx, 0.5 + dy), width: (0.5 - dy, 0.5 + dx) })
}

fn apply_borders_and_corners(n: &mut figcraft_doc::Node, props: &BTreeMap<String, String>, em: f64) {
    if let Some(r) = props.get("border-radius") {
        let vals: Vec<f64> = r.split_whitespace().filter_map(|t| parse_px_len(t, em)).collect();
        n.corners = match vals.as_slice() {
            [u] => Corners { radii: [u.max(0.0); 4], smoothing: 0.0 },
            [tl, tr, br, bl, ..] => Corners { radii: [tl.max(0.0), tr.max(0.0), br.max(0.0), bl.max(0.0)], smoothing: 0.0 },
            _ => n.corners,
        };
    }
    if let Some(border) = props.get("border") {
        let mut weight = 1.0;
        let mut color = None;
        let mut dashed = false;
        for tok in split_top_level_ws(border) {
            if tok.eq_ignore_ascii_case("none") {
                return;
            } else if tok.eq_ignore_ascii_case("dashed") {
                dashed = true;
            } else if let Some(px) = parse_px_len(tok, em) {
                weight = px;
            } else if let Some(c) = Color::parse_css(tok) {
                color = Some(c);
            }
        }
        if weight > 0.0
            && let Some(c) = color
        {
            n.strokes = vec![Paint::solid(c)];
            n.stroke = StrokeProps { weight, dashes: if dashed { vec![weight * 3.0, weight * 2.0] } else { Vec::new() }, ..StrokeProps::default() };
        }
    } else if let Some(c) = props.get("border-color").and_then(|s| Color::parse_css(s)) {
        let w = props.get("border-width").and_then(|s| parse_px_len(s, em)).unwrap_or(1.0);
        if w > 0.0 {
            n.strokes = vec![Paint::solid(c)];
            n.stroke.weight = w;
        }
    }
}

fn apply_effects_and_opacity(n: &mut figcraft_doc::Node, props: &BTreeMap<String, String>, em: f64) {
    if let Some(op) = props.get("opacity").and_then(|v| v.trim().parse::<f32>().ok()) {
        n.opacity = op.clamp(0.0, 1.0);
    }
    if let Some(sh) = props.get("box-shadow").and_then(|s| parse_box_shadow(s, em)) {
        n.effects.push(sh);
    }
}

fn parse_box_shadow(s: &str, em: f64) -> Option<Effect> {
    let first = split_top_level_commas(s).into_iter().next()?;
    let mut inset = false;
    let mut nums = Vec::new();
    let mut color = Color::new(0.0, 0.0, 0.0, 0.18);
    for tok in split_top_level_ws(first) {
        if tok.eq_ignore_ascii_case("none") {
            return None;
        } else if tok.eq_ignore_ascii_case("inset") {
            inset = true;
        } else if let Some(px) = parse_px_len(tok, em) {
            nums.push(px);
        } else if let Some(c) = Color::parse_css(tok) {
            color = c;
        }
    }
    if nums.len() < 2 {
        return None;
    }
    let shadow = Shadow {
        offset: (nums[0], nums[1]),
        radius: nums.get(2).copied().unwrap_or(4.0).max(0.0),
        spread: nums.get(3).copied().unwrap_or(0.0),
        color,
        ..Shadow::default()
    };
    let kind = if inset { EffectKind::InnerShadow(shadow) } else { EffectKind::DropShadow(shadow) };
    Some(Effect { kind, visible: true })
}

fn derive_text_style(tag: &str, prev: &InheritedText, props: &BTreeMap<String, String>) -> InheritedText {
    let mut next = prev.clone();
    match tag {
        "h1" => (next.size, next.weight) = (36.0, 700),
        "h2" => (next.size, next.weight) = (28.0, 700),
        "h3" => (next.size, next.weight) = (22.0, 600),
        "h4" => (next.size, next.weight) = (18.0, 600),
        "h5" => (next.size, next.weight) = (16.0, 600),
        "h6" => (next.size, next.weight) = (14.0, 600),
        "small" => next.size = 13.0,
        "strong" | "b" | "button" => next.weight = 600,
        "em" | "i" => next.italic = true,
        "ul" => next.list = ListStyle::Bulleted,
        "ol" => next.list = ListStyle::Numbered,
        _ => {}
    }
    if let Some(fam) = props.get("font-family") {
        let primary = fam.split(',').next().unwrap_or(fam).trim().trim_matches(['"', '\'']);
        if !primary.is_empty() && !matches!(primary, "sans-serif" | "serif" | "monospace" | "system-ui" | "inherit") {
            next.family = primary.to_string();
        }
    }
    if let Some(sz) = props.get("font-size").and_then(|v| parse_px_len(v, prev.size)) {
        next.size = sz.clamp(4.0, 512.0);
    }
    if let Some(w) = props.get("font-weight") {
        let wl = w.trim().to_ascii_lowercase();
        next.weight = match wl.as_str() {
            "normal" => 400,
            "bold" => 700,
            "lighter" => 300,
            "bolder" => 700,
            _ => wl.parse::<u16>().ok().map_or(next.weight, |n| n.clamp(100, 900)),
        };
    }
    if let Some(fs) = props.get("font-style") {
        next.italic = fs.to_ascii_lowercase().contains("italic");
    }
    if let Some(c) = props.get("color").and_then(|s| Color::parse_css(s)) {
        next.color = c;
    }
    if let Some(al) = props.get("text-align") {
        next.align = match al.trim().to_ascii_lowercase().as_str() {
            "center" => TextAlignH::Center,
            "right" | "end" => TextAlignH::Right,
            "justify" => TextAlignH::Justified,
            _ => TextAlignH::Left,
        };
    }
    if let Some(lh) = props.get("line-height") {
        let t = lh.trim();
        if let Some(pct) = t.strip_suffix('%').and_then(|v| v.parse::<f64>().ok()) {
            next.line_height = LineHeight::Percent(pct);
        } else if let Some(px) = parse_px_len(t, next.size) {
            next.line_height = LineHeight::Pixels(px);
        } else if let Ok(mult) = t.parse::<f64>() {
            next.line_height = LineHeight::Percent(mult * 100.0);
        }
    }
    if let Some(ls) = props.get("letter-spacing")
        && let Some(px) = parse_px_len(ls, next.size)
    {
        next.letter_spacing = LetterSpacing::Pixels(px);
    }
    next
}

fn font_style_name(weight: u16, italic: bool) -> String {
    let base = match weight {
        0..=249 => "Thin",
        250..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "Semi Bold",
        650..=749 => "Bold",
        _ => "Extra Bold",
    };
    if italic { if base == "Regular" { "Italic".into() } else { format!("{base} Italic") } } else { base.into() }
}

fn parse_text_decoration(props: &BTreeMap<String, String>) -> TextDecoration {
    let Some(d) = props.get("text-decoration") else { return TextDecoration::None };
    let dl = d.to_ascii_lowercase();
    if dl.contains("underline") {
        TextDecoration::Underline
    } else if dl.contains("line-through") {
        TextDecoration::Strikethrough
    } else {
        TextDecoration::None
    }
}

fn parse_text_case(props: &BTreeMap<String, String>) -> TextCase {
    match props.get("text-transform").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("uppercase") => TextCase::Upper,
        Some("lowercase") => TextCase::Lower,
        Some("capitalize") => TextCase::Title,
        _ => TextCase::Original,
    }
}

fn element_label(el: &HtmlElem) -> String {
    if let Some(aria) = el.attrs.get("aria-label").or_else(|| el.attrs.get("alt")).filter(|s| !s.trim().is_empty()) {
        return aria.trim().to_string();
    }
    if let Some(id) = el.id.as_deref().filter(|s| !s.is_empty()) {
        return id.to_string();
    }
    if let Some(cls) = el.classes.first().filter(|s| !s.is_empty()) {
        return cls.replace('-', " ");
    }
    let mut chars = el.tag.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().chain(chars).collect(),
        None => "Element".into(),
    }
}

fn parse_px_len(raw: &str, em: f64) -> Option<f64> {
    let t = raw.trim().to_ascii_lowercase();
    let v = if let Some(n) = t.strip_suffix("px") {
        n.trim().parse::<f64>().ok()?
    } else if let Some(n) = t.strip_suffix("rem") {
        n.trim().parse::<f64>().ok()? * 16.0
    } else if let Some(n) = t.strip_suffix("em") {
        n.trim().parse::<f64>().ok()? * em
    } else if let Some(n) = t.strip_suffix("pt") {
        n.trim().parse::<f64>().ok()? * (96.0 / 72.0)
    } else if t == "0" {
        0.0
    } else {
        return None;
    };
    v.is_finite().then_some(v)
}

// ------------------------------------------------------------------ CSS cascade & var() resolution

fn resolve_styles(el: &HtmlElem, styles: &Stylesheet) -> BTreeMap<String, String> {
    let mut merged: Vec<(u32, usize, String, String)> = Vec::new();
    for (order, rule) in styles.rules.iter().enumerate() {
        if let Some(spec) = selector_specificity(&rule.selector, el) {
            for (k, v) in &rule.decls {
                merged.push((spec, order, k.clone(), resolve_css_vars(v, &styles.vars)));
            }
        }
    }
    merged.sort_by_key(|(spec, order, _, _)| (*spec, *order));
    let mut out = BTreeMap::new();
    for (_, _, k, v) in merged {
        out.insert(k, v);
    }
    if let Some(inline) = el.attrs.get("style") {
        for (k, v) in parse_decl_list(inline) {
            out.insert(k, resolve_css_vars(&v, &styles.vars));
        }
    }
    out
}

fn resolve_css_vars(value: &str, vars: &BTreeMap<String, String>) -> String {
    let mut current = value.to_string();
    for _ in 0..4 {
        let Some(start) = current.find("var(") else { break };
        let after = &current[start + 4..];
        let mut depth = 1usize;
        let mut end_rel = None;
        for (i, ch) in after.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end_rel = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end_rel else { break };
        let inner = &after[..end];
        let (var_name, fallback) = match inner.split_once(',') {
            Some((n, fb)) => (n.trim(), Some(fb.trim())),
            None => (inner.trim(), None),
        };
        let replacement = vars.get(var_name).map(String::as_str).or(fallback).unwrap_or("");
        current = format!("{}{}{}", &current[..start], replacement, &after[end + 1..]);
    }
    current
}

fn selector_specificity(selector: &str, el: &HtmlElem) -> Option<u32> {
    // Skip interactive pseudo-classes (:hover, :active, :focus) and pseudo-elements (::before).
    if selector.contains(':') || selector.contains(' ') || selector.contains('>') || selector.contains('+') || selector.contains('~') {
        // Simple descendant selectors (`header .brand`) match on their rightmost compound part when simple.
        if selector.contains(':') {
            return None;
        }
        let last = selector.split_whitespace().last()?;
        if last.contains(['>', '+', '~']) {
            return None;
        }
        return compound_specificity(last, el);
    }
    compound_specificity(selector.trim(), el)
}

fn compound_specificity(compound: &str, el: &HtmlElem) -> Option<u32> {
    if compound == "*" {
        return Some(0);
    }
    let mut spec = 0u32;
    let mut rest = compound;
    if !rest.starts_with(['.', '#']) {
        let end = rest.find(['.', '#']).unwrap_or(rest.len());
        let tag = &rest[..end];
        if !tag.eq_ignore_ascii_case(&el.tag) {
            return None;
        }
        spec += 1;
        rest = &rest[end..];
    }
    while !rest.is_empty() {
        let kind = rest.chars().next()?;
        rest = &rest[kind.len_utf8()..];
        let end = rest.find(['.', '#']).unwrap_or(rest.len());
        let ident = &rest[..end];
        rest = &rest[end..];
        match kind {
            '.' => {
                if !el.classes.iter().any(|c| c == ident) {
                    return None;
                }
                spec += 10;
            }
            '#' => {
                if el.id.as_deref() != Some(ident) {
                    return None;
                }
                spec += 100;
            }
            _ => return None,
        }
    }
    Some(spec)
}

fn parse_stylesheet(css: &str, sheet: &mut Stylesheet) {
    let clean = strip_css_comments(css);
    parse_css_blocks(&clean, sheet);
}

fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(end) => rest = &rest[i + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn parse_css_blocks(css: &str, sheet: &mut Stylesheet) {
    let mut i = 0;
    let bytes = css.as_bytes();
    while i < bytes.len() {
        let Some(open_rel) = css[i..].find('{') else { break };
        let header = css[i..i + open_rel].trim();
        let body_start = i + open_rel + 1;
        let mut depth = 1usize;
        let mut cur = body_start;
        while cur < bytes.len() && depth > 0 {
            match bytes[cur] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            cur += 1;
        }
        let body_end = cur.saturating_sub(1);
        let body = css.get(body_start..body_end).unwrap_or("");
        i = cur;

        if header.starts_with('@') {
            // Skip @media overrides for base static import, or recurse into @layer/@supports.
            if header.starts_with("@layer") || header.starts_with("@supports") {
                parse_css_blocks(body, sheet);
            }
            continue;
        }
        let decls = parse_decl_list(body);
        for sel in header.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if sel == ":root" || sel.eq_ignore_ascii_case("html") {
                for (k, v) in &decls {
                    if k.starts_with("--") {
                        sheet.vars.insert(k.clone(), v.clone());
                    }
                }
            }
            sheet.rules.push(CssRule { selector: sel.to_string(), decls: decls.clone() });
        }
    }
}

fn parse_decl_list(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for item in split_top_level_char(body, ';') {
        let Some((k, v)) = item.split_once(':') else { continue };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim().trim_end_matches("!important").trim().to_string();
        if !key.is_empty() && !val.is_empty() {
            out.push((key, val));
        }
    }
    out
}

fn split_top_level_char(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0usize, None::<char>, 0usize);
    for (i, ch) in s.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    split_top_level_char(s, ',')
}

fn split_top_level_ws(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0usize, None::<char>, None::<usize>);
    for (i, ch) in s.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                start.get_or_insert(i);
                quote = Some(ch);
            }
            '(' => {
                start.get_or_insert(i);
                depth += 1;
            }
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(st) = start.take() {
                    out.push(&s[st..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

// ------------------------------------------------------------------ HTML5 parser

fn parse_html_document(input: &str, warnings: &mut Vec<String>) -> (Vec<HtmlNode>, Stylesheet, Option<String>) {
    let mut sheet = Stylesheet::default();
    let mut title = None;
    let mut stack: Vec<HtmlElem> = vec![HtmlElem { tag: "document".into(), ..Default::default() }];
    let mut pos = 0usize;
    let mut total_elems = 0usize;

    while pos < input.len() {
        let rest = &input[pos..];
        let Some(lt) = rest.find('<') else {
            push_text(&mut stack, rest);
            break;
        };
        if lt > 0 {
            push_text(&mut stack, &rest[..lt]);
        }
        pos += lt;
        let tag_rest = &input[pos..];

        if tag_rest.starts_with("<!--") {
            pos += tag_rest.find("-->").map_or(tag_rest.len(), |e| e + 3);
            continue;
        }
        if tag_rest.len() > 2 && tag_rest[..2].eq_ignore_ascii_case("<!") {
            pos += tag_rest.find('>').map_or(tag_rest.len(), |e| e + 1);
            continue;
        }
        let Some(gt) = find_tag_end(tag_rest) else {
            break;
        };
        let inside = tag_rest[1..gt].trim();
        pos += gt + 1;

        if let Some(closing) = inside.strip_prefix('/') {
            let close_name = closing.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            close_matching_tag(&mut stack, &close_name);
            continue;
        }

        let self_close = inside.ends_with('/');
        let inside = inside.trim_end_matches('/').trim();
        let (tag_name, attr_str) = split_tag_name(inside);
        if tag_name.is_empty() {
            continue;
        }

        // Raw text elements: `<style>` and `<script>`.
        if matches!(tag_name.as_str(), "style" | "script") && !self_close {
            let (raw_body, next_pos) = read_raw_element(&input[pos..], &tag_name);
            pos += next_pos;
            if tag_name == "style" {
                parse_stylesheet(&raw_body, &mut sheet);
            }
            continue;
        }
        if tag_name == "title" && !self_close {
            let (raw_body, next_pos) = read_raw_element(&input[pos..], "title");
            pos += next_pos;
            let decoded = normalize_ws(&decode_entities(&raw_body));
            if !decoded.is_empty() {
                title = Some(decoded);
            }
            continue;
        }

        total_elems += 1;
        if total_elems > MAX_DOM_NODES {
            warnings.push(format!("HTML element count capped at {MAX_DOM_NODES}"));
            break;
        }

        let attrs = parse_html_attrs(attr_str);
        let id = attrs.get("id").cloned().filter(|s| !s.is_empty());
        let classes = attrs.get("class").map(|c| c.split_whitespace().map(str::to_string).collect()).unwrap_or_default();
        let elem = HtmlElem { tag: tag_name.clone(), id, classes, attrs, children: Vec::new() };

        if self_close || is_void_html_tag(&tag_name) || stack.len() >= MAX_DOM_DEPTH {
            if let Some(top) = stack.last_mut() {
                top.children.push(HtmlNode::Element(elem));
            }
        } else {
            stack.push(elem);
        }
    }

    while stack.len() > 1 {
        if let Some(el) = stack.pop()
            && let Some(top) = stack.last_mut()
        {
            top.children.push(HtmlNode::Element(el));
        }
    }
    let root = stack.pop().map(|r| r.children).unwrap_or_default();
    (root, sheet, title)
}

fn push_text(stack: &mut [HtmlElem], raw: &str) {
    let decoded = decode_entities(raw);
    if !decoded.is_empty()
        && let Some(top) = stack.last_mut()
    {
        top.children.push(HtmlNode::Text(decoded));
    }
}

fn close_matching_tag(stack: &mut Vec<HtmlElem>, name: &str) {
    if name.is_empty() {
        return;
    }
    let Some(pos) = stack.iter().rposition(|e| e.tag == name).filter(|p| *p > 0) else {
        return;
    };
    while stack.len() > pos {
        if let Some(el) = stack.pop()
            && let Some(top) = stack.last_mut()
        {
            top.children.push(HtmlNode::Element(el));
        }
    }
}

fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote = None::<char>;
    for (i, ch) in s.char_indices().skip(1) {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
        } else if matches!(ch, '"' | '\'') {
            quote = Some(ch);
        } else if ch == '>' {
            return Some(i);
        }
    }
    None
}

fn split_tag_name(s: &str) -> (String, &str) {
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    (s[..end].to_ascii_lowercase(), s[end..].trim())
}

fn read_raw_element(s: &str, tag: &str) -> (String, usize) {
    let needle = format!("</{tag}");
    let lower = s.to_ascii_lowercase();
    if let Some(idx) = lower.find(&needle) {
        let after = &s[idx..];
        let close_gt = after.find('>').map_or(after.len(), |g| g + 1);
        (s[..idx].to_string(), idx + close_gt)
    } else {
        (s.to_string(), s.len())
    }
}

fn is_void_html_tag(tag: &str) -> bool {
    matches!(tag, "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "param" | "source" | "track" | "wbr")
}

fn parse_html_attrs(mut s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() {
            break;
        }
        let name_end = s.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(s.len());
        if name_end == 0 {
            s = &s[1..];
            continue;
        }
        let name = s[..name_end].to_ascii_lowercase();
        s = s[name_end..].trim_start();
        let val = if let Some(after_eq) = s.strip_prefix('=') {
            s = after_eq.trim_start();
            if let Some(q) = s.chars().next().filter(|c| matches!(c, '"' | '\'')) {
                let inner = &s[q.len_utf8()..];
                match inner.find(q) {
                    Some(end) => {
                        let v = &inner[..end];
                        s = &inner[end + q.len_utf8()..];
                        v
                    }
                    None => {
                        let v = inner;
                        s = "";
                        v
                    }
                }
            } else {
                let end = s.find(char::is_whitespace).unwrap_or(s.len());
                let v = &s[..end];
                s = &s[end..];
                v
            }
        } else {
            ""
        };
        out.insert(name, decode_entities(val));
    }
    out
}

fn collect_inline_text(children: &[HtmlNode]) -> String {
    let mut raw = String::new();
    collect_inline_rec(children, &mut raw);
    raw.lines().map(normalize_ws).collect::<Vec<_>>().join("\n").trim().to_string()
}

fn collect_inline_rec(children: &[HtmlNode], out: &mut String) {
    for c in children {
        match c {
            HtmlNode::Text(t) => out.push_str(t),
            HtmlNode::Element(el) if el.tag == "br" => out.push('\n'),
            HtmlNode::Element(el) => collect_inline_rec(&el.children, out),
        }
    }
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';').filter(|i| *i <= 12) {
            let ent = &after[..semi];
            if let Some(decoded) = resolve_entity(ent) {
                out.push_str(&decoded);
                rest = &after[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn resolve_entity(ent: &str) -> Option<String> {
    if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).map(|c| c.to_string());
    }
    if let Some(dec) = ent.strip_prefix('#') {
        return dec.parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string());
    }
    Some(
        match ent {
            "amp" => "&",
            "lt" => "<",
            "gt" => ">",
            "quot" => "\"",
            "apos" | "#39" => "'",
            "nbsp" => " ",
            "mdash" => "—",
            "ndash" => "–",
            "copy" => "©",
            "rarr" => "→",
            _ => return None,
        }
        .to_string(),
    )
}
