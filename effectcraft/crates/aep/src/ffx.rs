//! After Effects Animation Preset (`.ffx`) import and export.
//!
//! An After Effects `.ffx` preset is a `RIFX` `Egg!` (or `FXPr` / XML) chunk container holding a
//! layer property tree (`ADBE Abstract Layer` -> `LIST tdgp`) with any combination of
//! `ADBE Effect Parade`, `ADBE Transform Group`, `ADBE Text Properties` (`ADBE Text Animators`),
//! and optional `Pefl` / `EfdG` effect parameter definitions.
//!
//! [`import_ffx`] normalizes the preset's property groups into a single-layer project container and
//! runs the verified [`crate::import`] pipeline so every mapped effect, placeholder effect,
//! keyframe ease, spatial tangent, expression, and text animator converts with identical fidelity
//! and loss reporting.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use effectcraft_color::Label;
use effectcraft_project::{Comp, ItemKind, Layer, LayerSource, Node, Project, PropGroup, Property, Solid, build};
use effectcraft_time::{FrameRate, TICKS_PER_SECOND, Tick};

use crate::export::{AepExportError, ExportOptions};
use crate::import::{AepImportError, ImportOptions, ImportReport};
use crate::{AepError, AepFile, Body, Chunk, PropGroup as AepGroup, PropNode};

/// Why an `.ffx` animation preset could not be imported or exported.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FfxError {
    #[error("{0}")]
    Container(#[from] AepError),
    #[error("{0}")]
    Import(#[from] AepImportError),
    #[error("{0}")]
    Export(#[from] AepExportError),
    #[error("the file holds no animation preset properties, effects, or text animators")]
    NoPresetData,
}

/// Decoded contents of an After Effects `.ffx` animation preset, retimed relative to its earliest
/// keyframe so it can be applied at the composition's current-time indicator (CTI).
#[derive(Clone, Debug, PartialEq)]
pub struct FfxPreset {
    pub name: String,
    pub effects: Vec<PropGroup>,
    pub properties: Vec<(String, Property)>,
    pub text_animators: Vec<PropGroup>,
    pub report: ImportReport,
}

const LAYER_GROUP_NAMES: &[&str] =
    &["ADBE Abstract Layer", "ADBE Layer", "ADBE AV Layer", "ADBE Text Layer", "ADBE Vector Layer", "ADBE Camera Layer", "ADBE Light Layer"];

const TOP_GROUPS: &[&str] =
    &["ADBE Effect Parade", "ADBE Transform Group", "ADBE Text Properties", "ADBE Layer Styles", "ADBE Mask Parade", "ADBE Root Vectors Group"];

const TRANSFORM_MAP: &[(&str, &str)] = &[
    ("ADBE Anchor Point", "transform/anchor"),
    ("ADBE Position", "transform/position"),
    ("ADBE Scale", "transform/scale"),
    ("ADBE Rotate Z", "transform/rotation"),
    ("ADBE Rotate X", "transform/rotationX"),
    ("ADBE Rotate Y", "transform/rotationY"),
    ("ADBE Orientation", "transform/orientation"),
    ("ADBE Opacity", "transform/opacity"),
];

/// Imports an After Effects `.ffx` binary (`RIFX` `Egg!` / `FXPr`) or XML animation preset.
pub fn import_ffx(bytes: &[u8], layer_size: (u32, u32), frame_rate: FrameRate) -> Result<FfxPreset, FfxError> {
    let norm = normalize_rifx_form(bytes);
    let file = crate::parse_any(&norm)?;
    let w = layer_size.0.clamp(1, 30_000);
    let h = layer_size.1.clamp(1, 30_000);

    if has_nonempty_fold(&file) {
        let mut proj = Project::default();
        let opts = ImportOptions { allow_losses: true, ..ImportOptions::new(&|_| false) };
        if let Ok(res) = crate::import::import(&mut proj, &norm, &opts)
            && let Some(preset) = extract_from_imported_project(&proj, res.report, None)
        {
            return Ok(preset);
        }
    }

    let tdgp = extract_preset_tdgp(&file.chunks).ok_or(FfxError::NoPresetData)?;
    let stored_tr = stored_transform_paths(&tdgp);
    let has_text = tdgp_has_match(&tdgp, "ADBE Text Properties") || tdgp_has_match(&tdgp, "ADBE Text Animators");

    let scratch = build_shell_project((w, h), frame_rate, has_text);
    let shell_res = crate::export::export(&scratch, &ExportOptions { allow_losses: true })?;
    let mut shell_file = crate::parse(&shell_res.bytes)?.into_owned();
    copy_effect_defs(&file, &mut shell_file);
    splice_preset_into_shell(&mut shell_file, &tdgp)?;

    let merged_bytes = crate::write(&shell_file)?;
    let mut imported = Project::default();
    let opts = ImportOptions { allow_losses: true, ..ImportOptions::new(&|_| false) };
    let res = crate::import::import(&mut imported, &merged_bytes, &opts)?;
    let preset_name = extract_preset_name(&file.chunks).unwrap_or_else(|| "FFX Preset".to_string());
    let mut preset = extract_from_imported_project(&imported, res.report, Some(&stored_tr)).ok_or(FfxError::NoPresetData)?;
    preset.name = preset_name;
    Ok(preset)
}

/// Exports a [`Layer`]'s effects, transform keyframes/expressions, and text animators as a binary
/// `RIFX` `Egg!` `.ffx` animation preset container.
pub fn export_ffx(layer: &Layer, layer_size: (u32, u32), frame_rate: FrameRate) -> Result<Vec<u8>, FfxError> {
    let w = layer_size.0.clamp(1, 30_000);
    let h = layer_size.1.clamp(1, 30_000);
    let has_text = layer.props.group("text/animators").is_some();
    let mut proj = build_shell_project((w, h), frame_rate, has_text);
    let cid = proj.items.iter().find_map(|(id, it)| matches!(it.kind, ItemKind::Comp(_)).then_some(*id));
    if let Some(cid) = cid
        && let Some(comp) = proj.comp_mut(cid)
        && let Some(dst) = comp.layers.first_mut()
    {
        if let (Some(src_fx), Some(dst_fx)) = (layer.effects(), dst.props.sub_mut("effects")) {
            dst_fx.children = src_fx.children.clone();
        }
        if let (Some(src_tr), Some(dst_tr)) = (layer.transform(), dst.props.sub_mut("transform")) {
            for (_, path) in TRANSFORM_MAP {
                let key = path.strip_prefix("transform/").unwrap_or(path);
                if let (Some(sp), Some(dp)) = (src_tr.get(key), dst_tr.get_mut(key)) {
                    dp.value = sp.value.clone();
                    dp.keys = sp.keys.clone();
                    dp.expr = sp.expr.clone();
                }
            }
        }
        if let (Some(src_anim), Some(dst_anim)) = (layer.props.group("text/animators"), dst.props.group_mut("text/animators")) {
            dst_anim.children = src_anim.children.clone();
        }
    }
    bump_next_id(&mut proj);
    let exported = crate::export::export(&proj, &ExportOptions { allow_losses: true })?;
    let full = crate::parse(&exported.bytes)?.into_owned();
    let layr_tdgp = find_first_layr_tdgp(&full.chunks).ok_or(FfxError::NoPresetData)?;

    let mut chunks: Vec<Chunk<'static>> = Vec::new();
    for ch in &full.chunks {
        if ch.is(b"svap") || ch.is(b"head") || ch.is(b"nhed") || ch.is(b"nnhd") || ch.is_list(b"Pefl") || ch.is_list(b"EfdG") || ch.is_list(b"ExEn") {
            chunks.push(ch.clone());
        }
    }
    chunks.push(tdmn_chunk("ADBE Abstract Layer"));
    chunks.push(layr_tdgp);
    let ffx_file = AepFile { chunks, trailer: Cow::Borrowed(&[]) };
    Ok(crate::write(&ffx_file)?)
}

fn normalize_rifx_form(bytes: &[u8]) -> Cow<'_, [u8]> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFX" && &bytes[8..12] == b"FXPr" {
        let mut copy = bytes.to_vec();
        copy[8..12].copy_from_slice(b"Egg!");
        Cow::Owned(copy)
    } else {
        Cow::Borrowed(bytes)
    }
}

fn has_nonempty_fold(file: &AepFile<'_>) -> bool {
    file.chunks.iter().any(|c| c.is_list(b"Fold") && c.children().iter().any(|k| !k.is(b"fdta")))
}

fn chunk_bytes<'a>(c: &'a Chunk<'_>) -> Option<&'a [u8]> {
    match &c.body {
        Body::Data(b) => Some(b.as_ref()),
        _ => None,
    }
}

fn tdmn_text(c: &Chunk<'_>) -> Option<String> {
    if !c.is(b"tdmn") {
        return None;
    }
    let data = chunk_bytes(c)?;
    let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
    std::str::from_utf8(&data[..end]).ok().map(str::to_string)
}

fn tdmn_chunk(name: &str) -> Chunk<'static> {
    let mut b = Vec::with_capacity(40);
    let bytes = name.as_bytes();
    let n = bytes.len().min(39);
    b.extend_from_slice(&bytes[..n]);
    b.resize(40, 0);
    Chunk::data(*b"tdmn", b)
}

fn tdsn_chunk(name: &str) -> Chunk<'static> {
    Chunk { id: *b"tdsn", body: Body::Chunks(vec![Chunk::data(*b"Utf8", name.as_bytes().to_vec())]), pad: None }
}

fn wrap_tdgp(kids: Vec<Chunk<'static>>) -> Chunk<'static> {
    let mut body = vec![Chunk::data(*b"tdsb", 1u32.to_be_bytes().to_vec()), tdsn_chunk("")];
    body.extend(kids);
    body.push(tdmn_chunk("ADBE Group End"));
    Chunk::list(*b"tdgp", body)
}

fn find_first_layr_tdgp(chunks: &[Chunk<'_>]) -> Option<Chunk<'static>> {
    for c in chunks {
        if c.is_list(b"Layr")
            && let Some(tdgp) = c.child_list(b"tdgp")
        {
            return Some(tdgp.clone().into_owned());
        }
        if !c.children().is_empty()
            && let Some(found) = find_first_layr_tdgp(c.children())
        {
            return Some(found);
        }
    }
    None
}

fn extract_preset_name(chunks: &[Chunk<'_>]) -> Option<String> {
    for c in chunks {
        if c.is(b"tdsn")
            && let Body::Chunks(inner) = &c.body
            && let Some(u) = inner.first().and_then(chunk_bytes)
            && let Ok(s) = std::str::from_utf8(u)
            && !s.trim().is_empty()
            && s != "-_0_/-"
        {
            return Some(s.trim().to_string());
        }
    }
    None
}

fn extract_preset_tdgp(chunks: &[Chunk<'_>]) -> Option<Chunk<'static>> {
    if let Some(tdgp) = find_first_layr_tdgp(chunks) {
        return Some(tdgp);
    }
    let mut top_pairs: Vec<Chunk<'static>> = Vec::new();
    let mut effect_pairs: Vec<Chunk<'static>> = Vec::new();
    let mut i = 0;
    while i < chunks.len() {
        if let Some(m) = tdmn_text(&chunks[i])
            && let Some(next) = chunks.get(i + 1)
        {
            if LAYER_GROUP_NAMES.contains(&m.as_str()) && next.is_list(b"tdgp") {
                return Some(next.clone().into_owned());
            }
            if m == "ADBE Text Animators" && next.is_list(b"tdgp") {
                let text_props = wrap_tdgp(vec![tdmn_chunk(&m), next.clone().into_owned()]);
                top_pairs.push(tdmn_chunk("ADBE Text Properties"));
                top_pairs.push(text_props);
                i += 2;
                continue;
            }
            if TOP_GROUPS.contains(&m.as_str()) && next.is_list(b"tdgp") {
                top_pairs.push(chunks[i].clone().into_owned());
                top_pairs.push(next.clone().into_owned());
                i += 2;
                continue;
            }
            if next.is_list(b"sspc") {
                effect_pairs.push(chunks[i].clone().into_owned());
                effect_pairs.push(next.clone().into_owned());
                i += 2;
                continue;
            }
        } else if chunks[i].is_list(b"tdgp") {
            let owned = chunks[i].clone().into_owned();
            if TOP_GROUPS.iter().any(|g| tdgp_has_match(&owned, g)) {
                return Some(owned);
            }
            if owned.children().iter().any(|k| k.is_list(b"sspc")) {
                return Some(wrap_tdgp(vec![tdmn_chunk("ADBE Effect Parade"), owned]));
            }
        }
        i += 1;
    }
    if !effect_pairs.is_empty() {
        let mut parade_body = vec![Chunk::data(*b"tdsb", 1u32.to_be_bytes().to_vec()), tdsn_chunk(""), Chunk::data(*b"fnam", Vec::new())];
        parade_body.extend(effect_pairs);
        parade_body.push(tdmn_chunk("ADBE Group End"));
        top_pairs.push(tdmn_chunk("ADBE Effect Parade"));
        top_pairs.push(Chunk::list(*b"tdgp", parade_body));
    }
    (!top_pairs.is_empty()).then(|| wrap_tdgp(top_pairs))
}

fn tdgp_has_match(c: &Chunk<'_>, target: &str) -> bool {
    for k in c.children() {
        if tdmn_text(k).as_deref() == Some(target) {
            return true;
        }
        if !k.children().is_empty() && tdgp_has_match(k, target) {
            return true;
        }
    }
    false
}

fn stored_transform_paths(tdgp: &Chunk<'_>) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(root) = AepGroup::from_chunk(tdgp) else { return out };
    let Some(PropNode::Group(tr)) = root.get("ADBE Transform Group") else { return out };
    for (aep_name, ec_path) in TRANSFORM_MAP {
        let present = match tr.get(aep_name) {
            Some(PropNode::Property(p)) => p.is_animated() || p.flags() == Some(1) || p.expression().is_some(),
            Some(PropNode::Other(otst)) => otst.is_list(b"otst"),
            _ => false,
        };
        if present {
            out.insert((*ec_path).to_string());
        }
    }
    out
}

fn build_shell_project(size: (u32, u32), frame_rate: FrameRate, has_text: bool) -> Project {
    let mut proj = Project::default();
    let duration = Tick(TICKS_PER_SECOND * 10);
    let mut comp = Comp::new(size.0, size.1, frame_rate, duration);
    let source = if has_text {
        LayerSource::Text
    } else {
        let solid_id = proj.add_item(
            "Preset Solid",
            Label::None,
            None,
            ItemKind::Solid(Solid { width: size.0, height: size.1, color: [0.5, 0.5, 0.5], pixel_aspect: 1.0 }),
        );
        LayerSource::Solid { item: solid_id }
    };
    let layer = build::layer(&mut proj, &comp, "Preset Layer", source, size, None);
    comp.layers.push(layer);
    proj.add_item("Preset Comp", Label::None, None, ItemKind::Comp(Arc::new(comp)));
    bump_next_id(&mut proj);
    proj
}

fn bump_next_id(proj: &mut Project) {
    let mut max_id = proj.next_id;
    for (id, item) in &proj.items {
        max_id = max_id.max(id.0 + 1);
        if let ItemKind::Comp(c) = &item.kind {
            for l in c.layers.iter() {
                max_id = max_id.max(l.id.0 + 1);
                max_uid(&l.props, &mut max_id);
            }
        }
    }
    proj.next_id = max_id;
}

fn max_uid(g: &PropGroup, max_id: &mut u64) {
    *max_id = (*max_id).max(g.uid + 1);
    for child in &g.children {
        match child {
            Node::Prop(p) => *max_id = (*max_id).max(p.uid + 1),
            Node::Group(sub) => max_uid(sub, max_id),
        }
    }
}

fn copy_effect_defs(src: &AepFile<'_>, dst: &mut AepFile<'static>) {
    if let Some(pefl) = src.chunks.iter().find(|c| c.is_list(b"Pefl") && !c.children().is_empty())
        && let Some(slot) = dst.chunks.iter_mut().find(|c| c.is_list(b"Pefl"))
    {
        *slot = pefl.clone().into_owned();
    }
    if let Some(efdg) = src.chunks.iter().find(|c| c.is_list(b"EfdG") && !c.children().is_empty()) {
        if let Some(slot) = dst.chunks.iter_mut().find(|c| c.is_list(b"EfdG")) {
            *slot = efdg.clone().into_owned();
        } else if let Some(pos) = dst.chunks.iter().position(|c| c.is_list(b"Pefl")) {
            dst.chunks.insert(pos + 1, efdg.clone().into_owned());
        }
    }
}

fn splice_preset_into_shell(shell: &mut AepFile<'static>, preset_tdgp: &Chunk<'static>) -> Result<(), FfxError> {
    let layr = find_first_layr_mut(&mut shell.chunks).ok_or(FfxError::NoPresetData)?;
    let Body::List { children, .. } = &mut layr.body else { return Err(FfxError::NoPresetData) };
    let Some(shell_tdgp) = children.iter_mut().find(|c| c.is_list(b"tdgp")) else {
        return Err(FfxError::NoPresetData);
    };
    merge_tdgp(shell_tdgp, preset_tdgp);
    Ok(())
}

fn find_first_layr_mut<'a>(chunks: &'a mut [Chunk<'static>]) -> Option<&'a mut Chunk<'static>> {
    for c in chunks {
        if c.is_list(b"Layr") {
            return Some(c);
        }
        if let Body::List { children, .. } = &mut c.body
            && let Some(found) = find_first_layr_mut(children)
        {
            return Some(found);
        }
    }
    None
}

fn extract_named_pairs(tdgp: &Chunk<'static>) -> Vec<(String, Chunk<'static>)> {
    let mut out = Vec::new();
    let kids = tdgp.children();
    let mut i = 0;
    while i < kids.len() {
        if let Some(name) = tdmn_text(&kids[i]) {
            if name == "ADBE Group End" {
                break;
            }
            if let Some(val) = kids.get(i + 1) {
                out.push((name, val.clone()));
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn merge_tdgp(shell_tdgp: &mut Chunk<'static>, preset_tdgp: &Chunk<'static>) {
    let preset_pairs = extract_named_pairs(preset_tdgp);
    if preset_pairs.is_empty() {
        return;
    }
    let mut shell_pairs = extract_named_pairs(shell_tdgp);
    for (name, p_node) in preset_pairs {
        if name == "ADBE Text Properties"
            && let Some((_, existing)) = shell_pairs.iter_mut().find(|(n, _)| n == "ADBE Text Properties")
        {
            merge_tdgp(existing, &p_node);
            continue;
        }
        if let Some((_, slot)) = shell_pairs.iter_mut().find(|(n, _)| *n == name) {
            *slot = p_node;
        } else {
            shell_pairs.push((name, p_node));
        }
    }
    let mut body = vec![Chunk::data(*b"tdsb", 1u32.to_be_bytes().to_vec()), tdsn_chunk("")];
    for (name, node) in shell_pairs {
        body.push(tdmn_chunk(&name));
        body.push(node);
    }
    body.push(tdmn_chunk("ADBE Group End"));
    *shell_tdgp = Chunk::list(*b"tdgp", body);
}

fn extract_from_imported_project(proj: &Project, report: ImportReport, allowed_tr: Option<&HashSet<String>>) -> Option<FfxPreset> {
    let comp = proj.items.values().find_map(|it| match &it.kind {
        ItemKind::Comp(c) if !c.layers.is_empty() => Some(c.as_ref()),
        _ => None,
    })?;
    let layer = comp.layers.iter().find(|l| !matches!(l.source, LayerSource::Camera | LayerSource::Light { .. })).or_else(|| comp.layers.first())?;

    let mut effects: Vec<PropGroup> = layer.effects().map(|fx| fx.groups().cloned().collect()).unwrap_or_default();
    let mut text_animators: Vec<PropGroup> = layer.props.group("text/animators").map(|g| g.groups().cloned().collect()).unwrap_or_default();
    let mut properties: Vec<(String, Property)> = Vec::new();
    if let Some(tr) = layer.transform() {
        for (_, path) in TRANSFORM_MAP {
            let keep = match allowed_tr {
                Some(set) => set.contains(*path),
                None => {
                    let key = path.strip_prefix("transform/").unwrap_or(path);
                    tr.get(key).is_some_and(|p| !p.keys.is_empty() || p.expr.is_some())
                }
            };
            if keep {
                let key = path.strip_prefix("transform/").unwrap_or(path);
                if let Some(pr) = tr.get(key) {
                    properties.push(((*path).to_string(), pr.clone()));
                }
            }
        }
    }
    if effects.is_empty() && properties.is_empty() && text_animators.is_empty() {
        return None;
    }

    let first = earliest_preset_tick(&effects, &text_animators, &properties).unwrap_or(Tick::ZERO);
    if first != Tick::ZERO {
        for g in effects.iter_mut().chain(text_animators.iter_mut()) {
            shift_group_keys(g, first);
        }
        for (_, pr) in &mut properties {
            for k in &mut pr.keys {
                k.time = Tick(k.time.0.saturating_sub(first.0));
            }
        }
    }

    Some(FfxPreset { name: layer.name.clone(), effects, properties, text_animators, report })
}

fn earliest_preset_tick(effects: &[PropGroup], animators: &[PropGroup], props: &[(String, Property)]) -> Option<Tick> {
    let mut first: Option<Tick> = None;
    let mut consider = |t: Tick| {
        first = Some(first.map_or(t, |cur| cur.min(t)));
    };
    for g in effects.iter().chain(animators.iter()) {
        g.walk("", &mut |_, pr| {
            if let Some(t) = pr.keys.iter().map(|k| k.time).min() {
                consider(t);
            }
        });
    }
    for (_, pr) in props {
        if let Some(t) = pr.keys.iter().map(|k| k.time).min() {
            consider(t);
        }
    }
    first
}

fn shift_group_keys(g: &mut PropGroup, delta: Tick) {
    for child in &mut g.children {
        match child {
            Node::Prop(pr) => {
                for k in &mut pr.keys {
                    k.time = Tick(k.time.0.saturating_sub(delta.0));
                }
            }
            Node::Group(sub) => shift_group_keys(sub, delta),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{FfxError, export_ffx, import_ffx};
    use effectcraft_color::Label;
    use effectcraft_keyframe::{Keyframe, Value};
    use effectcraft_project::build::{self, Ids};
    use effectcraft_project::{Comp, Expression, ItemKind, LayerSource, Node, Project, Solid};
    use effectcraft_time::{FrameRate, TICKS_PER_SECOND, Tick};

    #[test]
    fn ffx_round_trips_effect_transform_keyframes_and_expression() {
        let mut proj = Project::default();
        let comp = Comp::new(1920, 1080, FrameRate::FPS_30, Tick(TICKS_PER_SECOND * 5));
        let solid_id =
            proj.add_item("Hero Solid", Label::None, None, ItemKind::Solid(Solid { width: 1920, height: 1080, color: [0.5, 0.5, 0.5], pixel_aspect: 1.0 }));
        let mut layer = build::layer(&mut proj, &comp, "Hero", LayerSource::Solid { item: solid_id }, (1920, 1080), None);
        let spec = effectcraft_effects::find("ec.blur.gaussian").unwrap();
        let mut fx_group = effectcraft_effects::instantiate(spec, &mut Ids(&mut proj.next_id), "Soft Blur", [1920.0, 1080.0]);
        fx_group.prop_mut("blurriness").unwrap().value = Value::Scalar(24.0);
        layer.props.sub_mut("effects").unwrap().children.push(Node::Group(fx_group));

        {
            let opacity = layer.props.prop_mut("transform/opacity").unwrap();
            opacity.keys = vec![Keyframe::new(Tick(TICKS_PER_SECOND), Value::Scalar(15.0)), Keyframe::new(Tick(TICKS_PER_SECOND * 2), Value::Scalar(95.0))];
            opacity.value = Value::Scalar(15.0);
        }
        {
            let rot = layer.props.prop_mut("transform/rotation").unwrap();
            rot.expr = Some(Expression { text: "time * 45".into(), enabled: true });
        }

        let ffx_bytes = export_ffx(&layer, (1920, 1080), FrameRate::FPS_30).expect("export ffx");
        assert_eq!(&ffx_bytes[0..4], b"RIFX");
        assert_eq!(&ffx_bytes[8..12], b"Egg!");
        assert!(crate::parse(&ffx_bytes).unwrap().chunks.iter().all(|c| !c.is_list(b"Fold")), "pure FFX has no Fold");

        let preset = import_ffx(&ffx_bytes, (1920, 1080), FrameRate::FPS_30).expect("import ffx");
        assert_eq!(preset.effects.len(), 1);
        assert_eq!(preset.effects[0].name, "Soft Blur");
        assert!((preset.effects[0].prop("blurriness").unwrap().value.as_f64() - 24.0).abs() < 1e-3);

        let (_, opacity) = preset.properties.iter().find(|(p, _)| p == "transform/opacity").expect("opacity imported");
        assert_eq!(opacity.keys.len(), 2);
        assert_eq!(opacity.keys[0].time, Tick::ZERO, "retimed relative to earliest keyframe");
        assert_eq!(opacity.keys[1].time, Tick(TICKS_PER_SECOND));
        assert!((opacity.keys[0].value.as_f64() - 15.0).abs() < 1e-3);
        assert!((opacity.keys[1].value.as_f64() - 95.0).abs() < 1e-3);

        let (_, rot) = preset.properties.iter().find(|(p, _)| p == "transform/rotation").expect("rotation imported");
        assert_eq!(rot.expr.as_ref().map(|e| e.text.as_str()), Some("time * 45"));

        // Also accepts FXPr RIFX form header.
        let mut fxpr = ffx_bytes.clone();
        fxpr[8..12].copy_from_slice(b"FXPr");
        let preset2 = import_ffx(&fxpr, (1920, 1080), FrameRate::FPS_30).expect("FXPr form");
        assert_eq!(preset2.effects.len(), 1);

        // Rejects empty/non-preset data cleanly.
        assert!(matches!(import_ffx(b"not a preset", (1920, 1080), FrameRate::FPS_30), Err(FfxError::Container(_))));
    }
}
