//! AGC node tree conversion: artboards, shapes (`rect`, `circle`, `ellipse`, `line`, `polygon`,
//! `path`, `compound`), multi-run `text`, `group`s, `clipPath` masks, `repeatGrid`, master symbols
//! and `syncRef` instances.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

use figcraft_doc::color::{Color, Paint};
use figcraft_doc::geom::kurbo::Shape as _;
use figcraft_doc::geom::{Affine, BezPath, Point};
use figcraft_doc::proto::{Overflow, ScrollPosition};
use figcraft_doc::{
    BoolOp, Corners, Document, LetterSpacing, LineHeight, MaskType, Node, NodeId, NodeKind, StyleRun, TextAlignH, TextAutoResize, TextCase,
    TextData, TextDecoration, TextStyle, VectorNetwork, WindingRule,
};
use serde_json::Value;
use zip::ZipArchive;

use super::archive::{AssetEntry, Budget, MAX_ASSET_BYTES, ParsedArchive};
use super::style::{
    bool_at, f64_at, parse_blend_mode, parse_color, parse_effects, parse_fills, parse_stroke, parse_transform, str_at,
};
use crate::FormatError;

const MAX_NODES: usize = 200_000;
const MAX_DEPTH: usize = 128;
const MAX_REPEAT_CELLS: usize = 256;

pub(super) struct ImportCtx<'a> {
    pub doc: Document,
    pub z: ZipArchive<Cursor<&'a [u8]>>,
    pub budget: Budget,
    pub assets: BTreeMap<String, AssetEntry>,
    pub loaded_images: BTreeMap<String, String>,
    pub loaded_videos: BTreeMap<String, (String, String)>,
    pub gradients: BTreeMap<String, Value>,
    pub clip_paths: BTreeMap<String, Value>,
    pub symbols: BTreeMap<String, NodeId>,
    pub artboard_map: BTreeMap<String, NodeId>,
    pub node_guid_map: BTreeMap<String, NodeId>,
    pub pending_interactions: Vec<(NodeId, Value)>,
    pub warnings: BTreeSet<String>,
    pub node_count: usize,
}

impl<'a> ImportCtx<'a> {
    pub fn warn(&mut self, msg: impl Into<String>) {
        if self.warnings.len() < 200 {
            self.warnings.insert(msg.into());
        }
    }

    pub fn asset_mime(&self, uid: &str) -> Option<&str> {
        self.assets.get(uid).map(|a| a.mime.as_str()).filter(|m| !m.is_empty())
    }

    pub fn read_asset_bytes(&mut self, uid: &str) -> Option<Vec<u8>> {
        let path = self
            .assets
            .get(uid)
            .map(|a| a.path.clone())
            .unwrap_or_else(|| format!("resources/{uid}"));
        self.budget.read_bytes(&mut self.z, &path, MAX_ASSET_BYTES).ok()
    }

    pub fn register_image_bytes(&mut self, uid: &str, raw: Vec<u8>) -> Option<String> {
        if let Some(existing) = self.loaded_images.get(uid) {
            return Some(existing.clone());
        }
        match crate::add_image(&mut self.doc, raw) {
            Ok((hash, _, _)) => {
                self.loaded_images.insert(uid.to_string(), hash.clone());
                Some(hash)
            }
            Err(e) => {
                self.warn(format!("Image resource `{uid}` could not be decoded ({e})"));
                None
            }
        }
    }

    pub fn load_video_asset(&mut self, uid: &str, poster_uid: Option<&str>) -> Option<(String, String)> {
        if let Some(existing) = self.loaded_videos.get(uid) {
            return Some(existing.clone());
        }
        let raw = self.read_asset_bytes(uid)?;
        self.register_video_bytes(uid, raw, poster_uid)
    }

    pub fn register_video_bytes(&mut self, uid: &str, raw: Vec<u8>, poster_uid: Option<&str>) -> Option<(String, String)> {
        if let Some(existing) = self.loaded_videos.get(uid) {
            return Some(existing.clone());
        }
        let poster_bytes = poster_uid.and_then(|p| self.read_asset_bytes(p));
        match crate::add_video(&mut self.doc, raw, poster_bytes) {
            Ok((vhash, phash, _, _, _)) => {
                self.loaded_videos.insert(uid.to_string(), (vhash.clone(), phash.clone()));
                Some((vhash, phash))
            }
            Err(e) => {
                self.warn(format!("Video resource `{uid}` could not be imported ({e})"));
                None
            }
        }
    }

    fn ingest_agc_resources(&mut self, agc: &Value) {
        let Some(res) = agc.get("resources") else { return };
        if let Some(grads) = res.get("gradients").and_then(Value::as_object) {
            for (k, v) in grads {
                self.gradients.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        if let Some(clips) = res.get("clipPaths").and_then(Value::as_object) {
            for (k, v) in clips {
                self.clip_paths.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }
}

pub(super) fn build_document(
    parsed: ParsedArchive,
    z: ZipArchive<Cursor<&[u8]>>,
    budget: Budget,
    archive_warnings: Vec<String>,
) -> Result<(Document, Vec<String>), FormatError> {
    let doc = Document::new(&parsed.title);
    let Some(page_id) = doc.first_page() else {
        return Err(FormatError::Corrupt("document missing initial page".into()));
    };
    let mut warnings = BTreeSet::new();
    for w in archive_warnings {
        warnings.insert(w);
    }

    let mut ctx = ImportCtx {
        doc,
        z,
        budget,
        assets: parsed.assets,
        loaded_images: BTreeMap::new(),
        loaded_videos: BTreeMap::new(),
        gradients: BTreeMap::new(),
        clip_paths: BTreeMap::new(),
        symbols: BTreeMap::new(),
        artboard_map: BTreeMap::new(),
        node_guid_map: BTreeMap::new(),
        pending_interactions: Vec::new(),
        warnings,
        node_count: 0,
    };

    if let Some(res_agc) = &parsed.resources_agc {
        ctx.ingest_agc_resources(res_agc);
        import_master_symbols(&mut ctx, res_agc, page_id);
    }

    let mut home_frame_id: Option<NodeId> = None;
    let mut next_auto_x = 0.0_f64;

    for ab in &parsed.artboards {
        ctx.ingest_agc_resources(&ab.agc);
        let root_artboard = find_agc_artboard_node(&ab.agc);
        let [x, y, w, h] = ab.bounds.unwrap_or_else(|| {
            let w = root_artboard
                .and_then(|n| f64_at(n, "width"))
                .unwrap_or(375.0)
                .clamp(1.0, 1_000_000.0);
            let h = root_artboard
                .and_then(|n| f64_at(n, "height"))
                .unwrap_or(812.0)
                .clamp(1.0, 1_000_000.0);
            let bx = next_auto_x;
            next_auto_x += w + 100.0;
            [bx, 0.0, w, h]
        });

        let frame_id = ctx.doc.alloc_id();
        let mut frame = Node::new(frame_id, NodeKind::Frame, ab.name.clone(), w, h);
        frame.clip = true;
        frame.transform = Affine::translate((x, y));

        if let Some(vh) = ab.viewport_height
            && h > vh + 0.5
        {
            frame.overflow = Overflow::Vertical;
        }

        if let Some(ab_node) = root_artboard {
            let fills = parse_fills(ab_node.get("style"), w, h, &mut ctx);
            if !fills.is_empty() {
                frame.fills = fills;
            } else {
                frame.fills = vec![Paint::solid(Color::WHITE)];
            }
            if let Some(agc_id) = str_at(ab_node, "id").filter(|s| !s.is_empty()) {
                ctx.artboard_map.insert(agc_id.to_string(), frame_id);
                ctx.node_guid_map.insert(agc_id.to_string(), frame_id);
            }
            record_interactions(&mut ctx, frame_id, ab_node);
        } else {
            frame.fills = vec![Paint::solid(Color::WHITE)];
        }

        if !ab.id.is_empty() {
            ctx.artboard_map.insert(ab.id.clone(), frame_id);
            ctx.node_guid_map.insert(ab.id.clone(), frame_id);
        }
        if ab.is_home || parsed.home_artboard.as_deref() == Some(ab.id.as_str()) {
            home_frame_id = Some(frame_id);
        }

        if ctx.doc.insert(frame, page_id, None).is_err() {
            continue;
        }
        ctx.node_count += 1;

        let children = artboard_children(&ab.agc, root_artboard);
        for child in children {
            let _ = import_node(&mut ctx, child, frame_id, 0);
        }
    }

    if let Some(pb_agc) = &parsed.pasteboard_agc {
        ctx.ingest_agc_resources(pb_agc);
        if let Some(children) = pb_agc.get("children").and_then(Value::as_array) {
            for child in children {
                let _ = import_node(&mut ctx, child, page_id, 0);
            }
        }
    }

    super::proto::resolve_interactions(&mut ctx, page_id, home_frame_id);

    let issues: Vec<String> = ctx.warnings.into_iter().collect();
    Ok((ctx.doc, issues))
}

fn find_agc_artboard_node(agc: &Value) -> Option<&Value> {
    if str_at(agc, "type") == Some("artboard") {
        return Some(agc);
    }
    let children = agc.get("children")?.as_array()?;
    children
        .iter()
        .find(|c| str_at(c, "type") == Some("artboard"))
        .or_else(|| children.first().filter(|c| c.get("artboard").is_some()))
}

fn artboard_children<'a>(agc: &'a Value, root_ab: Option<&'a Value>) -> &'a [Value] {
    if let Some(ab) = root_ab {
        if let Some(inner) = ab.get("artboard").and_then(|a| a.get("children")).and_then(Value::as_array) {
            return inner.as_slice();
        }
        if let Some(ch) = ab.get("children").and_then(Value::as_array) {
            return ch.as_slice();
        }
    }
    agc.get("children").and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn import_master_symbols(ctx: &mut ImportCtx<'_>, res_agc: &Value, page_id: NodeId) {
    let symbols_val = res_agc
        .get("resources")
        .and_then(|r| r.get("meta"))
        .and_then(|m| m.get("ux"))
        .and_then(|u| u.get("symbols"))
        .or_else(|| res_agc.get("meta").and_then(|m| m.get("ux")).and_then(|u| u.get("symbols")));

    let mut list: Vec<(String, &Value)> = Vec::new();
    if let Some(arr) = symbols_val.and_then(Value::as_array) {
        for sym in arr.iter().take(2_000) {
            if let Some(id) = str_at(sym, "id")
                .or_else(|| sym.get("meta").and_then(|m| m.get("ux")).and_then(|u| str_at(u, "symbolId")))
                .filter(|s| !s.is_empty())
            {
                list.push((id.to_string(), sym));
            }
        }
    } else if let Some(obj) = symbols_val.and_then(Value::as_object) {
        for (id, sym) in obj.iter().take(2_000) {
            list.push((id.clone(), sym));
        }
    }

    let mut cursor_x = 0.0_f64;
    for (sym_id, sym) in list {
        let comp_id = ctx.doc.alloc_id();
        let name = str_at(sym, "name").unwrap_or("Component").to_string();
        let w = f64_at(sym, "width").unwrap_or(100.0).clamp(1.0, 100_000.0);
        let h = f64_at(sym, "height").unwrap_or(100.0).clamp(1.0, 100_000.0);
        let mut comp = Node::new(
            comp_id,
            NodeKind::Component { description: String::new(), props: BTreeMap::new(), prop_order: Vec::new() },
            name,
            w,
            h,
        );
        comp.fills.clear();
        comp.transform = Affine::translate((cursor_x, -400.0));
        if ctx.doc.insert(comp, page_id, None).is_err() {
            continue;
        }
        ctx.node_count += 1;

        let children = sym
            .get("group")
            .and_then(|g| g.get("children"))
            .or_else(|| sym.get("children"))
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for ch in children {
            let _ = import_node(ctx, ch, comp_id, 1);
        }
        let _ = ctx.doc.fit_to_children(comp_id);
        if let Some(n) = ctx.doc.node_mut(comp_id) {
            let fitted_w = n.width.max(1.0);
            n.transform = Affine::translate((cursor_x, -(n.height.max(40.0) + 200.0)));
            cursor_x += fitted_w + 80.0;
        }
        ctx.symbols.insert(sym_id.clone(), comp_id);
        ctx.node_guid_map.insert(sym_id, comp_id);
    }
}

fn record_interactions(ctx: &mut ImportCtx<'_>, node_id: NodeId, v: &Value) {
    let ints = v
        .get("meta")
        .and_then(|m| m.get("ux"))
        .and_then(|u| u.get("interactions"))
        .or_else(|| v.get("interactions"));
    if let Some(arr) = ints.and_then(Value::as_array)
        && !arr.is_empty()
    {
        ctx.pending_interactions.push((node_id, Value::Array(arr.clone())));
    }
}

fn import_node(ctx: &mut ImportCtx<'_>, v: &Value, parent: NodeId, depth: usize) -> Option<NodeId> {
    if depth > MAX_DEPTH {
        ctx.warn("Layers nested deeper than 128 levels were skipped");
        return None;
    }
    if ctx.node_count >= MAX_NODES {
        ctx.warn("Layer budget reached while importing Adobe XD file; remaining layers were skipped");
        return None;
    }

    // Check local clipPathResources inside `meta.ux`
    if let Some(clips) = v
        .get("meta")
        .and_then(|m| m.get("ux"))
        .and_then(|u| u.get("clipPathResources"))
    {
        if let Some(id) = str_at(clips, "id") {
            ctx.clip_paths.insert(id.to_string(), clips.clone());
        } else if let Some(arr) = clips.as_array() {
            for c in arr {
                if let Some(id) = str_at(c, "id") {
                    ctx.clip_paths.insert(id.to_string(), c.clone());
                }
            }
        }
    }

    let ty = str_at(v, "type").unwrap_or("group");

    // Master symbol instance (`syncRef`, `syncSourceGuid`, or `meta.ux.symbolId`)
    let sync_id = str_at(v, "syncRef")
        .or_else(|| str_at(v, "syncSourceGuid"))
        .or_else(|| (ty == "syncRef").then(|| str_at(v, "ref").or_else(|| str_at(v, "guid"))).flatten())
        .or_else(|| {
            v.get("meta")
                .and_then(|m| m.get("ux"))
                .and_then(|u| {
                    str_at(u, "syncSourceGuid").or_else(|| {
                        str_at(u, "symbolId")
                            .filter(|_| v.get("syncSourceGuid").is_some() || ty == "syncRef")
                    })
                })
        });
    if let Some(sym_guid) = sync_id {
        if let Some(&comp_id) = ctx.symbols.get(sym_guid) {
            return instantiate_symbol(ctx, v, comp_id, parent);
        }
        // If `v` carries inline group children, fall through to import them as a group; otherwise warn.
        if v.get("group").is_none() && v.get("children").is_none() {
            ctx.warn(format!("Symbol instance referencing unknown symbol `{sym_guid}` was skipped"));
            return None;
        }
    }

    // Repeat Grid (`meta.ux.repeatGrid`)
    if let Some(rg) = v
        .get("meta")
        .and_then(|m| m.get("ux"))
        .and_then(|u| u.get("repeatGrid"))
    {
        return import_repeat_grid(ctx, v, rg, parent, depth);
    }

    match ty {
        "shape" => import_shape(ctx, v, parent, depth),
        "text" => import_text(ctx, v, parent),
        "group" | "artboard" => import_group(ctx, v, parent, depth),
        other => {
            if v.get("shape").is_some() {
                import_shape(ctx, v, parent, depth)
            } else if v.get("text").is_some() {
                import_text(ctx, v, parent)
            } else if v.get("group").is_some() || v.get("children").is_some() {
                import_group(ctx, v, parent, depth)
            } else {
                ctx.warn(format!("Unsupported Adobe XD node type `{other}` was skipped"));
                None
            }
        }
    }
}

fn apply_common_node_props(ctx: &mut ImportCtx<'_>, node: &mut Node, v: &Value, local_offset: (f64, f64)) {
    let style = v.get("style");
    node.visible = bool_at(v, "visible")
        .or_else(|| style.and_then(|s| bool_at(s, "visible")))
        .unwrap_or(true);
    node.locked = v
        .get("meta")
        .and_then(|m| m.get("ux"))
        .and_then(|u| bool_at(u, "locked"))
        .unwrap_or(false);
    node.opacity = style
        .and_then(|s| f64_at(s, "opacity"))
        .or_else(|| f64_at(v, "opacity"))
        .unwrap_or(1.0)
        .clamp(0.0, 1.0) as f32;
    node.blend = parse_blend_mode(style.and_then(|s| str_at(s, "blendMode")).or_else(|| str_at(v, "blendMode")));

    let base_xf = parse_transform(v.get("transform"));
    node.transform = if local_offset == (0.0, 0.0) {
        base_xf
    } else {
        base_xf * Affine::translate(local_offset)
    };

    if !matches!(node.kind, NodeKind::Group) {
        let fills = parse_fills(style, node.width, node.height, ctx);
        if matches!(node.kind, NodeKind::Text(_)) {
            if !fills.is_empty() {
                node.fills = fills;
            }
        } else {
            node.fills = fills;
        }
        let (strokes, stroke_props) = parse_stroke(style, ctx);
        node.strokes = strokes;
        node.stroke = stroke_props;
    }
    node.effects = parse_effects(style, ctx);

    if let Some(ux) = v.get("meta").and_then(|m| m.get("ux")) {
        if bool_at(ux, "fixed").unwrap_or(false)
            || ux.get("constraints").and_then(|c| bool_at(c, "fixed")).unwrap_or(false)
        {
            node.scroll_position = ScrollPosition::Fixed;
        }
        if let Some(scroll) = str_at(ux, "scrollingType") {
            node.overflow = match scroll {
                "vertical" => Overflow::Vertical,
                "horizontal" => Overflow::Horizontal,
                "panning" | "both" => Overflow::Both,
                _ => Overflow::None,
            };
        }
    }
}

fn register_node(ctx: &mut ImportCtx<'_>, node: Node, v: &Value, parent: NodeId) -> Option<NodeId> {
    let id = node.id;
    if let Some(guid) = str_at(v, "id").filter(|s| !s.is_empty()) {
        ctx.node_guid_map.insert(guid.to_string(), id);
    }
    record_interactions(ctx, id, v);
    ctx.doc.insert(node, parent, None).ok()?;
    ctx.node_count += 1;
    Some(id)
}

fn instantiate_symbol(ctx: &mut ImportCtx<'_>, v: &Value, comp_id: NodeId, parent: NodeId) -> Option<NodeId> {
    let (comp_name, comp_w, comp_h, comp_children) = {
        let comp = ctx.doc.node(comp_id)?;
        (comp.name.clone(), comp.width, comp.height, comp.children.clone())
    };
    let inst_id = ctx.doc.alloc_id();
    let name = str_at(v, "name").unwrap_or(&comp_name).to_string();
    let mut inst = Node::new(
        inst_id,
        NodeKind::Instance { component: comp_id, props: BTreeMap::new() },
        name,
        comp_w,
        comp_h,
    );
    inst.fills.clear();
    apply_common_node_props(ctx, &mut inst, v, (0.0, 0.0));
    let id = register_node(ctx, inst, v, parent)?;

    for child_id in comp_children {
        clone_component_subtree(ctx, child_id, id, 0);
    }
    Some(id)
}

fn clone_component_subtree(ctx: &mut ImportCtx<'_>, src_id: NodeId, dst_parent: NodeId, depth: usize) {
    if depth > MAX_DEPTH || ctx.node_count >= MAX_NODES {
        return;
    }
    let Some((mut cloned, src_children)) = ctx.doc.node(src_id).map(|n| (n.clone(), n.children.clone())) else {
        return;
    };
    let new_id = ctx.doc.alloc_id();
    cloned.id = new_id;
    cloned.parent = None;
    cloned.children.clear();
    cloned.source = Some(src_id);
    if ctx.doc.insert(cloned, dst_parent, None).is_ok() {
        ctx.node_count += 1;
        for grand in src_children {
            clone_component_subtree(ctx, grand, new_id, depth + 1);
        }
    }
}

fn import_shape(ctx: &mut ImportCtx<'_>, v: &Value, parent: NodeId, depth: usize) -> Option<NodeId> {
    let sh = v.get("shape").unwrap_or(v);
    let shape_ty = str_at(sh, "type").unwrap_or("rect");
    let default_name = match shape_ty {
        "rect" => "Rectangle",
        "circle" | "ellipse" => "Ellipse",
        "line" => "Line",
        "polygon" => "Polygon",
        "compound" => "Boolean group",
        _ => "Path",
    };
    let name = str_at(v, "name").unwrap_or(default_name).to_string();

    match shape_ty {
        "rect" => {
            let x = f64_at(sh, "x").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
            let y = f64_at(sh, "y").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
            let w = f64_at(sh, "width").unwrap_or(100.0).clamp(0.01, 1_000_000.0);
            let h = f64_at(sh, "height").unwrap_or(100.0).clamp(0.01, 1_000_000.0);
            let mut node = Node::new(ctx.doc.alloc_id(), NodeKind::Rectangle, name, w, h);
            if let Some(r_val) = sh.get("r") {
                if let Some(r) = r_val.as_f64().filter(|n| n.is_finite()) {
                    node.corners = Corners { radii: [r.max(0.0); 4], smoothing: 0.0 };
                } else if let Some(arr) = r_val.as_array() {
                    let rad = |idx: usize| {
                        arr.get(idx)
                            .and_then(Value::as_f64)
                            .filter(|n| n.is_finite())
                            .unwrap_or(0.0)
                            .max(0.0)
                    };
                    node.corners = Corners {
                        radii: [rad(0), rad(1), rad(2), rad(3)],
                        smoothing: 0.0,
                    };
                }
            }
            apply_common_node_props(ctx, &mut node, v, (x, y));
            register_node(ctx, node, v, parent)
        }
        "circle" => {
            let r = f64_at(sh, "r").unwrap_or(50.0).clamp(0.01, 500_000.0);
            let cx = f64_at(sh, "cx").unwrap_or(r).clamp(-1_000_000.0, 1_000_000.0);
            let cy = f64_at(sh, "cy").unwrap_or(r).clamp(-1_000_000.0, 1_000_000.0);
            let mut node = Node::new(
                ctx.doc.alloc_id(),
                NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 },
                name,
                r * 2.0,
                r * 2.0,
            );
            apply_common_node_props(ctx, &mut node, v, (cx - r, cy - r));
            register_node(ctx, node, v, parent)
        }
        "ellipse" => {
            let rx = f64_at(sh, "rx").unwrap_or(50.0).clamp(0.01, 500_000.0);
            let ry = f64_at(sh, "ry").unwrap_or(50.0).clamp(0.01, 500_000.0);
            let cx = f64_at(sh, "cx").unwrap_or(rx).clamp(-1_000_000.0, 1_000_000.0);
            let cy = f64_at(sh, "cy").unwrap_or(ry).clamp(-1_000_000.0, 1_000_000.0);
            let mut node = Node::new(
                ctx.doc.alloc_id(),
                NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 },
                name,
                rx * 2.0,
                ry * 2.0,
            );
            apply_common_node_props(ctx, &mut node, v, (cx - rx, cy - ry));
            register_node(ctx, node, v, parent)
        }
        "line" => {
            let x1 = f64_at(sh, "x1").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
            let y1 = f64_at(sh, "y1").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
            let x2 = f64_at(sh, "x2").unwrap_or(100.0).clamp(-1_000_000.0, 1_000_000.0);
            let y2 = f64_at(sh, "y2").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
            let (dx, dy) = (x2 - x1, y2 - y1);
            let len = dx.hypot(dy).max(0.01);
            let angle = dy.atan2(dx);
            let mut node = Node::new(ctx.doc.alloc_id(), NodeKind::Line, name, len, 0.0);
            node.fills.clear();
            apply_common_node_props(ctx, &mut node, v, (0.0, 0.0));
            node.transform = parse_transform(v.get("transform")) * Affine::translate((x1, y1)) * Affine::rotate(angle);
            register_node(ctx, node, v, parent)
        }
        "polygon" => {
            if let Some(count) = sh
                .get("uxdesign#cornerCount")
                .or_else(|| sh.get("cornerCount"))
                .and_then(Value::as_u64)
                .map(|c| c.clamp(3, 60) as u32)
            {
                let w = f64_at(sh, "uxdesign#width").or_else(|| f64_at(sh, "width")).unwrap_or(100.0).clamp(1.0, 1_000_000.0);
                let h = f64_at(sh, "uxdesign#height").or_else(|| f64_at(sh, "height")).unwrap_or(100.0).clamp(1.0, 1_000_000.0);
                let star_ratio = f64_at(sh, "uxdesign#starRatio").or_else(|| f64_at(sh, "starRatio"));
                let kind = match star_ratio.filter(|r| *r > 0.0 && *r < 0.999) {
                    Some(r) => NodeKind::Star { count, inner_ratio: r.clamp(0.01, 1.0) },
                    None => NodeKind::Polygon { count },
                };
                let mut node = Node::new(ctx.doc.alloc_id(), kind, name, w, h);
                apply_common_node_props(ctx, &mut node, v, (0.0, 0.0));
                return register_node(ctx, node, v, parent);
            }
            if let Some(pts) = sh.get("points").and_then(Value::as_array) {
                let mut bez = BezPath::new();
                for (i, pt) in pts.iter().take(10_000).enumerate() {
                    let px = f64_at(pt, "x").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
                    let py = f64_at(pt, "y").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
                    if i == 0 {
                        bez.move_to(Point::new(px, py));
                    } else {
                        bez.line_to(Point::new(px, py));
                    }
                }
                bez.close_path();
                return build_vector_from_bez(ctx, bez, WindingRule::NonZero, name, v, parent);
            }
            None
        }
        "compound" => {
            if let Some(children) = sh.get("children").and_then(Value::as_array).filter(|c| !c.is_empty()) {
                let op = match str_at(sh, "operation").unwrap_or("union").to_ascii_lowercase().as_str() {
                    "subtract" | "minus-front" => BoolOp::Subtract,
                    "intersect" => BoolOp::Intersect,
                    "exclude" | "xor" => BoolOp::Exclude,
                    _ => BoolOp::Union,
                };
                let id = ctx.doc.alloc_id();
                let mut node = Node::new(id, NodeKind::BooleanOp { op }, name, 100.0, 100.0);
                apply_common_node_props(ctx, &mut node, v, (0.0, 0.0));
                let bool_id = register_node(ctx, node, v, parent)?;
                for ch in children {
                    let _ = import_node(ctx, ch, bool_id, depth + 1);
                }
                let _ = ctx.doc.fit_to_children(bool_id);
                return Some(bool_id);
            }
            if let Some(d) = str_at(sh, "path") {
                let winding = parse_winding(sh);
                return import_svg_path_shape(ctx, d, winding, name, v, parent);
            }
            None
        }
        _ => {
            if let Some(d) = str_at(sh, "path") {
                let winding = parse_winding(sh);
                import_svg_path_shape(ctx, d, winding, name, v, parent)
            } else {
                ctx.warn(format!("Unsupported Adobe XD shape `{shape_ty}` was skipped"));
                None
            }
        }
    }
}

fn parse_winding(sh: &Value) -> WindingRule {
    match str_at(sh, "winding").unwrap_or("nonzero").to_ascii_lowercase().as_str() {
        "evenodd" | "even-odd" => WindingRule::EvenOdd,
        _ => WindingRule::NonZero,
    }
}

fn import_svg_path_shape(
    ctx: &mut ImportCtx<'_>,
    d: &str,
    winding: WindingRule,
    name: String,
    v: &Value,
    parent: NodeId,
) -> Option<NodeId> {
    if d.len() > (4 << 20) {
        ctx.warn(format!("Vector path `{name}` exceeds 4 MiB and was skipped"));
        return None;
    }
    match figcraft_doc::geom::svg_path(d) {
        Ok(bez) => build_vector_from_bez(ctx, bez, winding, name, v, parent),
        Err(e) => {
            ctx.warn(format!("Malformed SVG path on `{name}` skipped ({e})"));
            None
        }
    }
}

fn build_vector_from_bez(
    ctx: &mut ImportCtx<'_>,
    mut bez: BezPath,
    winding: WindingRule,
    name: String,
    v: &Value,
    parent: NodeId,
) -> Option<NodeId> {
    let bb = bez.bounding_box();
    let (ox, oy) = if bb.x0.is_finite() && bb.y0.is_finite() { (bb.x0, bb.y0) } else { (0.0, 0.0) };
    if ox != 0.0 || oy != 0.0 {
        bez.apply_affine(Affine::translate((-ox, -oy)));
    }
    let w = bb.width().max(0.01).min(1_000_000.0);
    let h = bb.height().max(0.01).min(1_000_000.0);
    let mut net = VectorNetwork::from_path(&bez);
    for r in &mut net.regions {
        r.winding = winding;
    }
    let mut node = Node::new(ctx.doc.alloc_id(), NodeKind::Vector { network: net }, name, w, h);
    apply_common_node_props(ctx, &mut node, v, (ox, oy));
    register_node(ctx, node, v, parent)
}

fn import_text(ctx: &mut ImportCtx<'_>, v: &Value, parent: NodeId) -> Option<NodeId> {
    let tx = v.get("text")?;
    let raw = str_at(tx, "rawText").unwrap_or("");
    let chars_norm = raw.replace("\r\n", "\n").replace('\r', "\n");

    let style_obj = v.get("style");
    let base_style = parse_text_style(style_obj, None, ctx);
    let frame_obj = tx.get("frame");
    let frame_ty = frame_obj.and_then(|f| str_at(f, "type")).unwrap_or("positioned");

    let (auto_resize, box_w, box_h, y_shift) = match frame_ty {
        "area" => (
            TextAutoResize::None,
            frame_obj.and_then(|f| f64_at(f, "width")).unwrap_or(200.0).clamp(1.0, 100_000.0),
            frame_obj.and_then(|f| f64_at(f, "height")).unwrap_or(60.0).clamp(1.0, 100_000.0),
            0.0,
        ),
        "autoHeight" => (
            TextAutoResize::Height,
            frame_obj.and_then(|f| f64_at(f, "width")).unwrap_or(200.0).clamp(1.0, 100_000.0),
            (base_style.font_size * 1.4).max(16.0),
            0.0,
        ),
        _ => {
            let approx_w = (chars_norm.chars().count().max(1) as f64 * base_style.font_size * 0.58).clamp(8.0, 100_000.0);
            let approx_h = (base_style.font_size * 1.25).max(12.0);
            // Point text in AGC anchors `transform.ty` on the first baseline.
            (TextAutoResize::WidthAndHeight, approx_w, approx_h, -base_style.font_size * 0.85)
        }
    };

    let align_h = match style_obj
        .and_then(|s| s.get("textAttributes"))
        .and_then(|ta| str_at(ta, "paragraphAlign").or_else(|| str_at(ta, "textAlign")))
        .or_else(|| style_obj.and_then(|s| str_at(s, "textAlign")))
        .unwrap_or("left")
        .to_ascii_lowercase()
        .as_str()
    {
        "center" | "middle" => TextAlignH::Center,
        "right" | "end" => TextAlignH::Right,
        "justify" | "justified" => TextAlignH::Justified,
        _ => TextAlignH::Left,
    };

    let runs = parse_text_runs(tx, v, &chars_norm, &base_style, ctx);

    let mut td = TextData {
        characters: chars_norm.clone(),
        style: base_style,
        runs,
        auto_resize,
        align_h,
        ..TextData::default()
    };
    if let Some(ps) = style_obj
        .and_then(|s| s.get("textAttributes"))
        .and_then(|ta| f64_at(ta, "paragraphSpacing"))
    {
        td.paragraph_spacing = ps.clamp(0.0, 10_000.0);
    }

    let fallback_name: String = chars_norm.lines().next().unwrap_or("Text").chars().take(48).collect();
    let name = str_at(v, "name")
        .filter(|s| !s.is_empty())
        .unwrap_or(if fallback_name.is_empty() { "Text" } else { &fallback_name })
        .to_string();

    let fx = frame_obj.and_then(|f| f64_at(f, "x")).unwrap_or(0.0);
    let fy = frame_obj.and_then(|f| f64_at(f, "y")).unwrap_or(0.0) + y_shift;

    let mut node = Node::new(ctx.doc.alloc_id(), NodeKind::Text(td), name, box_w, box_h);
    node.fills = vec![Paint::solid(Color::BLACK)];
    apply_common_node_props(ctx, &mut node, v, (fx, fy));
    register_node(ctx, node, v, parent)
}

fn parse_text_style(style_obj: Option<&Value>, base: Option<&TextStyle>, ctx: &mut ImportCtx<'_>) -> TextStyle {
    let mut out = base.cloned().unwrap_or_default();
    let Some(st) = style_obj else { return out };
    if let Some(font) = st.get("font") {
        if let Some(fam) = str_at(font, "family").filter(|s| !s.is_empty()) {
            out.font_family = fam.to_string();
        } else if let Some(ps) = str_at(font, "postscript").filter(|s| !s.is_empty()) {
            let fam = ps.split('-').next().unwrap_or(ps);
            out.font_family = fam.to_string();
        }
        if let Some(fstyle) = str_at(font, "style").filter(|s| !s.is_empty()) {
            out.font_style = fstyle.to_string();
        }
        if let Some(sz) = f64_at(font, "size").filter(|s| *s > 0.0) {
            out.font_size = sz.clamp(1.0, 10_000.0);
        }
    }
    if let Some(sz) = f64_at(st, "fontSize").filter(|s| *s > 0.0) {
        out.font_size = sz.clamp(1.0, 10_000.0);
    }
    if let Some(fam) = str_at(st, "fontFamily").filter(|s| !s.is_empty()) {
        out.font_family = fam.to_string();
    }
    if let Some(fstyle) = str_at(st, "fontStyle").filter(|s| !s.is_empty()) {
        out.font_style = fstyle.to_string();
    }

    let ta = st.get("textAttributes").unwrap_or(st);
    if let Some(lh) = f64_at(ta, "lineHeight").or_else(|| f64_at(st, "lineSpacing")).filter(|v| *v > 0.0) {
        out.line_height = LineHeight::Pixels(lh.clamp(1.0, 10_000.0));
    }
    if let Some(ls) = f64_at(ta, "letterSpacing").or_else(|| f64_at(ta, "charSpacing")).or_else(|| f64_at(st, "charSpacing")) {
        // Adobe XD `charSpacing` is often 1/1000 em (`tracking`), whereas `letterSpacing` is px.
        if ta.get("letterSpacing").is_some() {
            out.letter_spacing = LetterSpacing::Pixels(ls.clamp(-1000.0, 1000.0));
        } else {
            out.letter_spacing = LetterSpacing::Percent((ls / 10.0).clamp(-500.0, 1000.0));
        }
    }
    if bool_at(ta, "underline").unwrap_or(false) {
        out.decoration = TextDecoration::Underline;
    } else if bool_at(ta, "strikethrough").or_else(|| bool_at(ta, "lineThrough")).unwrap_or(false) {
        out.decoration = TextDecoration::Strikethrough;
    }
    if let Some(tc) = str_at(ta, "textTransform") {
        out.case = match tc.to_ascii_lowercase().as_str() {
            "uppercase" | "upper" => TextCase::Upper,
            "lowercase" | "lower" => TextCase::Lower,
            "titlecase" | "capitalize" | "title" => TextCase::Title,
            _ => TextCase::Original,
        };
    }
    if base.is_some() {
        let fills = parse_fills(Some(st), 100.0, 100.0, ctx);
        if !fills.is_empty() {
            out.fills = Some(fills);
        } else if let Some(c) = st.get("fill").and_then(parse_color) {
            out.fills = Some(vec![Paint::solid(c)]);
        }
    }
    out
}

fn parse_text_runs(
    tx: &Value,
    node_val: &Value,
    chars: &str,
    base_style: &TextStyle,
    ctx: &mut ImportCtx<'_>,
) -> Vec<StyleRun> {
    let total_chars = chars.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    // Build UTF-16 code unit -> char index lookup so Adobe XD UTF-16 `length` spans map cleanly.
    let mut utf16_to_char = Vec::with_capacity(chars.len() + 1);
    for (ci, ch) in chars.chars().enumerate() {
        for _ in 0..ch.len_utf16() {
            utf16_to_char.push(ci);
        }
    }
    let total_utf16 = utf16_to_char.len();
    let map_u16 = |u: usize| -> usize {
        if u >= total_utf16 { total_chars } else { utf16_to_char[u] }
    };

    let range_list = node_val
        .get("meta")
        .and_then(|m| m.get("ux"))
        .and_then(|u| u.get("rangedStyles"))
        .or_else(|| tx.get("styleRanges"))
        .or_else(|| tx.get("rangedStyles"))
        .or_else(|| {
            tx.get("paragraphs")
                .and_then(Value::as_array)
                .and_then(|p| p.first())
                .and_then(|p0| p0.get("styleRanges"))
        });

    let Some(arr) = range_list.and_then(Value::as_array) else { return Vec::new() };
    let mut runs = Vec::new();
    let mut cursor_u16 = 0usize;
    for entry in arr.iter().take(4096) {
        let start_u16 = entry
            .get("startIndex")
            .or_else(|| entry.get("start"))
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(cursor_u16);
        let len_u16 = entry
            .get("length")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or_else(|| total_utf16.saturating_sub(start_u16));
        let end_u16 = start_u16.saturating_add(len_u16).min(total_utf16);
        cursor_u16 = end_u16;
        let start = map_u16(start_u16);
        let end = map_u16(end_u16);
        if end <= start {
            continue;
        }
        let st_src = entry.get("style").unwrap_or(entry);
        let run_style = parse_text_style(Some(st_src), Some(base_style), ctx);
        if &run_style != base_style {
            runs.push(StyleRun { start, end, style: run_style });
        }
    }
    runs.sort_by_key(|r| (r.start, r.end));
    // Clamp any overlapping ranges so `TextData::runs` stays strictly non-overlapping.
    let mut clean: Vec<StyleRun> = Vec::with_capacity(runs.len());
    for mut r in runs {
        if let Some(prev) = clean.last()
            && r.start < prev.end
        {
            r.start = prev.end;
        }
        if r.end > r.start && r.end <= total_chars {
            clean.push(r);
        }
    }
    clean
}

fn import_group(ctx: &mut ImportCtx<'_>, v: &Value, parent: NodeId, depth: usize) -> Option<NodeId> {
    let name = str_at(v, "name").unwrap_or("Group").to_string();
    let ux = v.get("meta").and_then(|m| m.get("ux"));
    let scroll_ty = ux.and_then(|u| str_at(u, "scrollingType"));

    // Scroll groups need a clipped Frame so Prototype scrolling works natively.
    let is_scroll_group = scroll_ty.is_some_and(|s| s != "none" && !s.is_empty());
    let group_id = ctx.doc.alloc_id();
    let (kind, w, h) = if is_scroll_group {
        let vw = ux.and_then(|u| f64_at(u, "viewportWidth")).unwrap_or(300.0).clamp(1.0, 100_000.0);
        let vh = ux.and_then(|u| f64_at(u, "viewportHeight")).unwrap_or(300.0).clamp(1.0, 100_000.0);
        (NodeKind::Frame, vw, vh)
    } else {
        (NodeKind::Group, 100.0, 100.0)
    };
    let mut node = Node::new(group_id, kind, name, w, h);
    node.fills.clear();
    if is_scroll_group {
        node.clip = true;
    }
    apply_common_node_props(ctx, &mut node, v, (0.0, 0.0));
    let id = register_node(ctx, node, v, parent)?;

    // Check if this group has a clipPath mask (`style.clipPath.ref`).
    if let Some(clip_ref) = v
        .get("style")
        .and_then(|s| s.get("clipPath"))
        .and_then(|cp| str_at(cp, "ref"))
        && let Some(clip_def) = ctx.clip_paths.get(clip_ref).cloned()
    {
        let mask_children = clip_def
            .get("children")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        if let Some(first_mask) = mask_children.first() {
            if let Some(mask_id) = import_node(ctx, first_mask, id, depth + 1)
                && let Some(mn) = ctx.doc.node_mut(mask_id)
            {
                mn.is_mask = true;
                mn.mask_type = MaskType::Vector;
            }
        } else if clip_def.get("shape").is_some()
            && let Some(mask_id) = import_shape(ctx, &clip_def, id, depth + 1)
            && let Some(mn) = ctx.doc.node_mut(mask_id)
        {
            mn.is_mask = true;
            mn.mask_type = MaskType::Vector;
        }
    }

    let children = v
        .get("group")
        .and_then(|g| g.get("children"))
        .or_else(|| v.get("children"))
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    for ch in children {
        let _ = import_node(ctx, ch, id, depth + 1);
    }

    if !is_scroll_group {
        let _ = ctx.doc.fit_to_children(id);
    }
    Some(id)
}

fn import_repeat_grid(ctx: &mut ImportCtx<'_>, v: &Value, rg: &Value, parent: NodeId, depth: usize) -> Option<NodeId> {
    let name = str_at(v, "name").unwrap_or("Repeat Grid").to_string();
    let gw = f64_at(rg, "width").unwrap_or(320.0).clamp(1.0, 100_000.0);
    let gh = f64_at(rg, "height").unwrap_or(240.0).clamp(1.0, 100_000.0);
    let pad_x = f64_at(rg, "paddingX").unwrap_or(16.0).clamp(0.0, 10_000.0);
    let pad_y = f64_at(rg, "paddingY").unwrap_or(16.0).clamp(0.0, 10_000.0);

    let frame_id = ctx.doc.alloc_id();
    let mut frame = Node::new(frame_id, NodeKind::Frame, name, gw, gh);
    frame.fills.clear();
    frame.clip = true;
    apply_common_node_props(ctx, &mut frame, v, (0.0, 0.0));
    let id = register_node(ctx, frame, v, parent)?;

    let children = v
        .get("group")
        .and_then(|g| g.get("children"))
        .or_else(|| v.get("children"))
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);

    if children.len() > 1 {
        for ch in children.iter().take(MAX_REPEAT_CELLS) {
            let _ = import_node(ctx, ch, id, depth + 1);
        }
        return Some(id);
    }

    // Single template cell: tile across `gw × gh` using `cellWidth`/`cellHeight` + `paddingX`/`paddingY`.
    if let Some(template) = children.first() {
        let first_cell = import_node(ctx, template, id, depth + 1)?;
        let (cw, ch_h) = ctx
            .doc
            .node(first_cell)
            .map_or((100.0, 100.0), |n| (n.width.max(1.0), n.height.max(1.0)));
        let cw = f64_at(rg, "cellWidth").unwrap_or(cw).max(1.0);
        let ch_h = f64_at(rg, "cellHeight").unwrap_or(ch_h).max(1.0);
        let step_x = (cw + pad_x).max(1.0);
        let step_y = (ch_h + pad_y).max(1.0);
        let cols = ((gw + pad_x) / step_x).floor().max(1.0) as usize;
        let rows = ((gh + pad_y) / step_y).floor().max(1.0) as usize;
        let mut count = 1usize;
        for r in 0..rows {
            for c in 0..cols {
                if r == 0 && c == 0 {
                    continue;
                }
                if count >= MAX_REPEAT_CELLS {
                    break;
                }
                if let Some(cell_id) = import_node(ctx, template, id, depth + 1)
                    && let Some(cell_node) = ctx.doc.node_mut(cell_id)
                {
                    cell_node.transform = Affine::translate((c as f64 * step_x, r as f64 * step_y)) * cell_node.transform;
                }
                count += 1;
            }
        }
    }
    Some(id)
}
