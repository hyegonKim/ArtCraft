//! Authoring dynamic block parameters, actions, visibility states and lookup tables on a block
//! definition (or `*Model_Space` while editing inside `BEDIT`).
//!
//! Parameters and actions are stored directly as DXF evaluation-graph objects
//! (`ACAD_ENHANCEDBLOCK` → `ACAD_EVALUATION_GRAPH`, `BLOCK*PARAMETER`, `BLOCK*GRIP`,
//! `BLOCK*ACTION`) in [`Drawing::foreign`], matching AutoCAD's representation so authored dynamic
//! blocks evaluate immediately on [`crate::Insert`] references and round-trip through DXF and DWG.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cadcraft_geom::{Bounds2, Vec2, Vec3};

use super::{ActionKind, DynamicBlock, GripPart, LookupColumn, Parameter, ParameterKind, ValueSet, VisibilityState, dynamic_block, sections};
use crate::foreign::{Foreign, ForeignObject};
use crate::{Drawing, EntityStore, Handle, RawTag};

const ETAG_APP: &str = "AcDbBlockRepETag";

/// Specification for authoring a new dynamic block parameter.
#[derive(Clone, Debug)]
pub enum AuthorParamSpec {
    Point { at: Vec3, label: String, description: String, show_properties: bool },
    Linear { base: Vec3, end: Vec3, label: String, description: String, values: ValueSet, from_midpoint: bool, show_properties: bool },
    Polar { base: Vec3, end: Vec3, label: String, description: String, show_properties: bool },
    Xy { base: Vec3, end: Vec3, label: String, description: String, show_properties: bool },
    Rotation { base: Vec3, end: Vec3, label: String, description: String, show_properties: bool },
    Flip { base: Vec3, end: Vec3, label: String, description: String, show_properties: bool },
    Visibility { at: Vec3, label: String, description: String, states: Vec<String>, show_properties: bool },
    Lookup { at: Vec3, label: String, description: String, show_properties: bool },
}

/// Specification for authoring a new dynamic block action.
#[derive(Clone, Debug)]
pub enum AuthorActionSpec {
    Move {
        name: String,
        parameter_node: i32,
        end: GripPart,
        selection: Vec<Handle>,
        at: Vec3,
        multiplier: f64,
        angle_offset: f64,
    },
    Stretch {
        name: String,
        parameter_node: i32,
        end: GripPart,
        frame: Vec<Vec2>,
        selection: Vec<Handle>,
        at: Vec3,
        multiplier: f64,
        angle_offset: f64,
    },
    Scale {
        name: String,
        parameter_node: i32,
        selection: Vec<Handle>,
        at: Vec3,
    },
    Rotate {
        name: String,
        parameter_node: i32,
        selection: Vec<Handle>,
        at: Vec3,
    },
    Flip {
        name: String,
        parameter_node: i32,
        selection: Vec<Handle>,
        at: Vec3,
    },
    Lookup {
        name: String,
        parameter_node: i32,
        columns: Vec<LookupColumn>,
        rows: Vec<Vec<String>>,
        at: Vec3,
    },
}

/// Visibility state authoring operation on a block's visibility parameter.
#[derive(Clone, Debug)]
pub enum VisibilityOp {
    /// Add a new visibility state (optionally copying visibility from `copy_from` or current default).
    AddState { name: String, copy_from: Option<String> },
    /// Rename an existing visibility state.
    RenameState { old_name: String, new_name: String },
    /// Delete an existing visibility state (at least one state must remain).
    DeleteState { name: String },
    /// Move `name` to index 0 (making it the default visibility state).
    SetDefault { name: String },
    /// Show or hide `handles` in visibility `state` (adding them to `controlled` if needed).
    SetEntitiesVisibility { state: String, handles: Vec<Handle>, visible: bool },
}

fn tag(code: i32, value: impl Into<String>) -> RawTag {
    RawTag { code, value: value.into() }
}

fn fmt_f64(v: f64) -> String {
    if !v.is_finite() || v.abs() < 1e-12 { "0".to_string() } else { format!("{v}") }
}

fn push_pt(tags: &mut Vec<RawTag>, code: i32, p: Vec3) {
    tags.push(tag(code, fmt_f64(p.x)));
    tags.push(tag(code + 10, fmt_f64(p.y)));
    tags.push(tag(code + 20, fmt_f64(p.z)));
}

fn eval_header(handle: Handle, graph: Handle, node: i32, element_name: &str) -> Vec<RawTag> {
    vec![
        tag(5, handle.hex()),
        tag(330, graph.hex()),
        tag(100, "AcDbEvalExpr"),
        tag(90, node.to_string()),
        tag(98, "33"),
        tag(99, "232"),
        tag(100, "AcDbBlockElement"),
        tag(300, element_name),
        tag(98, "33"),
        tag(99, "232"),
        tag(1071, "0"),
    ]
}

fn grip_object(handle: Handle, graph: Handle, node: i32, kind: &str, at: Vec3) -> ForeignObject {
    let mut tags = eval_header(handle, graph, node, "Grip");
    tags.push(tag(100, "AcDbBlockGrip"));
    tags.push(tag(91, "0"));
    tags.push(tag(92, "0"));
    push_pt(&mut tags, 1010, at);
    tags.push(tag(280, "1"));
    tags.push(tag(93, "-1"));
    ForeignObject { handle, kind: kind.to_string(), tags }
}

/// Ensure `block_name` has a `BLOCK_RECORD` target handle, extension dictionary, and
/// `ACAD_EVALUATION_GRAPH` in `d.foreign`, returning `(xdict_handle, graph_handle)`.
fn ensure_graph(d: &mut Drawing, block_name: &str) -> (Handle, Handle) {
    let key_name = block_name.trim().to_ascii_uppercase();
    let rec_key = ("BLOCK_RECORD".to_string(), key_name.clone());
    if let Some(xd) = d.foreign.records.get(&rec_key).and_then(|r| r.xdict)
        && let Some(gh) = find_graph_in_dict(&d.foreign, xd)
    {
        ensure_block_target(d, &key_name, xd);
        return (xd, gh);
    }
    let br_h = existing_block_target(&d.foreign, &key_name).unwrap_or_else(|| d.new_handle());
    let xd_h = d.foreign.records.get(&rec_key).and_then(|r| r.xdict).unwrap_or_else(|| d.new_handle());
    let gr_h = d.new_handle();

    let f = Arc::make_mut(&mut d.foreign);
    f.targets.insert(br_h, rec_key.clone());
    f.records.entry(rec_key).or_default().xdict = Some(xd_h);

    if let Some(dict) = f.objects.iter_mut().find(|o| o.handle == xd_h) {
        dict.tags.push(tag(3, "ACAD_ENHANCEDBLOCK"));
        dict.tags.push(tag(360, gr_h.hex()));
    } else {
        f.objects.push(ForeignObject {
            handle: xd_h,
            kind: "DICTIONARY".into(),
            tags: vec![
                tag(5, xd_h.hex()),
                tag(330, br_h.hex()),
                tag(100, "AcDbDictionary"),
                tag(280, "1"),
                tag(281, "1"),
                tag(3, "ACAD_ENHANCEDBLOCK"),
                tag(360, gr_h.hex()),
            ],
        });
    }

    f.objects.push(ForeignObject { handle: gr_h, kind: "ACAD_EVALUATION_GRAPH".into(), tags: build_graph_tags(gr_h, xd_h, &[]) });
    (xd_h, gr_h)
}

fn existing_block_target(f: &Foreign, upper_block: &str) -> Option<Handle> {
    f.targets.iter().find(|(_, (k, n))| k == "BLOCK_RECORD" && n == upper_block).map(|(h, _)| *h)
}

fn ensure_block_target(d: &mut Drawing, upper_block: &str, xdict: Handle) {
    if existing_block_target(&d.foreign, upper_block).is_some() {
        return;
    }
    let owner = d
        .foreign
        .objects
        .iter()
        .find(|o| o.handle == xdict)
        .and_then(|o| o.tags.iter().find(|t| t.code == 330))
        .and_then(|t| Handle::parse_hex(&t.value));
    let br_h = owner.filter(|h| h.0 != 0).unwrap_or_else(|| d.new_handle());
    let f = Arc::make_mut(&mut d.foreign);
    f.targets.insert(br_h, ("BLOCK_RECORD".into(), upper_block.to_string()));
}

fn find_graph_in_dict(f: &Foreign, xdict: Handle) -> Option<Handle> {
    let dict = f.objects.iter().find(|o| o.handle == xdict)?;
    let mut it = dict.tags.iter();
    while let Some(t) = it.next() {
        if t.code == 3 && t.value.eq_ignore_ascii_case("ACAD_ENHANCEDBLOCK") {
            let gh = it.next().filter(|n| n.code == 350 || n.code == 360).and_then(|n| Handle::parse_hex(&n.value))?;
            if f.objects.iter().any(|o| o.handle == gh && o.kind.eq_ignore_ascii_case("ACAD_EVALUATION_GRAPH")) {
                return Some(gh);
            }
        }
    }
    None
}

fn build_graph_tags(graph_h: Handle, xdict_h: Handle, members: &[(i32, Handle)]) -> Vec<RawTag> {
    let next_id = members.iter().map(|(n, _)| *n).max().unwrap_or(0).saturating_add(1);
    let mut tags = vec![
        tag(5, graph_h.hex()),
        tag(102, "{ACAD_REACTORS"),
        tag(330, xdict_h.hex()),
        tag(102, "}"),
        tag(330, xdict_h.hex()),
        tag(100, "AcDbEvalGraph"),
        tag(96, next_id.to_string()),
        tag(97, "1"),
    ];
    for (idx, (node_id, member_h)) in members.iter().enumerate() {
        tags.push(tag(91, idx.to_string()));
        tags.push(tag(93, "32"));
        tags.push(tag(95, node_id.to_string()));
        tags.push(tag(360, member_h.hex()));
        for _ in 0..4 {
            tags.push(tag(92, "-1"));
        }
    }
    tags
}

fn read_graph_members(f: &Foreign, graph_h: Handle) -> Vec<(i32, Handle)> {
    let Some(g) = f.objects.iter().find(|o| o.handle == graph_h) else {
        return Vec::new();
    };
    let by_h: HashMap<Handle, &ForeignObject> = f.objects.iter().map(|o| (o.handle, o)).collect();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut pending_node: Option<i32> = None;
    for t in &g.tags {
        if t.code == 95 {
            pending_node = t.value.trim().parse::<i32>().ok();
        } else if t.code == 360
            && let Some(mh) = Handle::parse_hex(&t.value)
            && seen.insert(mh)
            && let Some(obj) = by_h.get(&mh)
        {
            let secs = sections(&obj.tags);
            let expr_node = super::section(&secs, "AcDbEvalExpr").iter().find(|x| x.code == 90).and_then(|x| x.value.trim().parse::<i32>().ok());
            let node = expr_node.or(pending_node).unwrap_or(1);
            out.push((node, mh));
            pending_node = None;
        }
    }
    out
}

fn next_node_id(members: &[(i32, Handle)]) -> i32 {
    members.iter().map(|(n, _)| *n).max().unwrap_or(0).saturating_add(1).max(1)
}

fn append_graph_members(d: &mut Drawing, xdict_h: Handle, graph_h: Handle, new_objs: Vec<(i32, ForeignObject)>) {
    let mut members = read_graph_members(&d.foreign, graph_h);
    let new_handles: Vec<Handle> = new_objs.iter().map(|(_, o)| o.handle).collect();
    for (n, o) in &new_objs {
        members.push((*n, o.handle));
    }
    let f = Arc::make_mut(&mut d.foreign);
    for (_, o) in new_objs {
        f.objects.push(o);
    }
    if let Some(g) = f.objects.iter_mut().find(|o| o.handle == graph_h) {
        g.tags = build_graph_tags(graph_h, xdict_h, &members);
    }
    // Any existing visibility parameter in this graph marks newly added parameters/grips/actions
    // active in all visibility states by default so their grips display immediately.
    let member_set: HashSet<Handle> = members.iter().map(|(_, h)| *h).collect();
    for o in &mut f.objects {
        if member_set.contains(&o.handle) && o.kind.eq_ignore_ascii_case("BLOCKVISIBILITYPARAMETER") {
            add_active_to_visibility_tags(&mut o.tags, &new_handles);
        }
    }
}

fn add_active_to_visibility_tags(tags: &mut Vec<RawTag>, extra_active: &[Handle]) {
    if extra_active.is_empty() {
        return;
    }
    let secs = sections(tags);
    let at = super::Cursor::new(super::section(&secs, "AcDbBlock1PtParameter")).point(1010);
    let grip = super::Cursor::new(super::section(&secs, "AcDbBlock1PtParameter")).int(93);
    let node = super::Cursor::new(super::section(&secs, "AcDbEvalExpr")).int(90);
    let elem = super::Cursor::new(super::section(&secs, "AcDbBlockElement")).string(300);
    let show = super::Cursor::new(super::section(&secs, "AcDbBlockParameter")).int(280) != 0;
    let owner = tags.iter().find(|t| t.code == 330).and_then(|t| Handle::parse_hex(&t.value)).unwrap_or(Handle(0));
    let self_h = tags.iter().find(|t| t.code == 5).and_then(|t| Handle::parse_hex(&t.value)).unwrap_or(Handle(0));
    let ParameterKind::Visibility { label, description, controlled, mut states, .. } =
        super::visibility(at, super::section(&secs, "AcDbBlockVisibilityParameter"))
    else {
        return;
    };
    for st in &mut states {
        for h in extra_active {
            if *h != self_h && !st.active.contains(h) {
                st.active.push(*h);
            }
        }
    }
    *tags = build_visibility_tags((self_h, owner, node, grip), (&elem, &label, &description, show), at, &controlled, &states);
}

fn build_visibility_tags(
    ids: (Handle, Handle, i32, i32),
    meta: (&str, &str, &str, bool),
    at: Vec3,
    controlled: &[Handle],
    states: &[VisibilityState],
) -> Vec<RawTag> {
    let (handle, graph, node, grip_node) = ids;
    let (elem, label, description, show_properties) = meta;
    let mut tags = eval_header(handle, graph, node, elem);
    tags.extend([
        tag(100, "AcDbBlockParameter"),
        tag(280, if show_properties { "1" } else { "0" }),
        tag(281, "0"),
        tag(100, "AcDbBlock1PtParameter"),
    ]);
    push_pt(&mut tags, 1010, at);
    tags.extend([
        tag(93, grip_node.to_string()),
        tag(170, "1"),
        tag(171, "0"),
        tag(100, "AcDbBlockVisibilityParameter"),
        tag(281, "1"),
        tag(301, label),
        tag(302, description),
        tag(91, "0"),
        tag(93, controlled.len().to_string()),
    ]);
    for h in controlled {
        tags.push(tag(331, h.hex()));
    }
    tags.push(tag(92, states.len().to_string()));
    for st in states {
        tags.push(tag(303, &st.name));
        tags.push(tag(94, st.visible.len().to_string()));
        for vh in &st.visible {
            tags.push(tag(332, vh.hex()));
        }
        tags.push(tag(95, st.active.len().to_string()));
        for ah in &st.active {
            tags.push(tag(333, ah.hex()));
        }
    }
    tags
}

fn push_two_pt_parameter(tags: &mut Vec<RawTag>, show_properties: bool, base: Vec3, end: Vec3, grips: &[i32], from_midpoint: bool) {
    tags.extend([
        tag(100, "AcDbBlockParameter"),
        tag(280, if show_properties { "1" } else { "0" }),
        tag(281, "0"),
        tag(100, "AcDbBlock2PtParameter"),
    ]);
    push_pt(tags, 1010, base);
    push_pt(tags, 1011, end);
    tags.push(tag(170, grips.len().to_string()));
    for i in 0..4 {
        let g = grips.get(i).copied().unwrap_or(0);
        tags.push(tag(91, g.to_string()));
    }
    tags.extend([tag(171, "0"), tag(172, "0"), tag(173, "0"), tag(174, "0"), tag(177, if from_midpoint { "1" } else { "0" })]);
}

fn push_value_set(tags: &mut Vec<RawTag>, vs: &ValueSet) {
    tags.push(tag(307, ""));
    tags.push(tag(96, vs.flags.to_string()));
    tags.push(tag(141, fmt_f64(vs.minimum)));
    tags.push(tag(142, fmt_f64(vs.maximum)));
    tags.push(tag(143, fmt_f64(vs.increment)));
    tags.push(tag(175, vs.list.len().to_string()));
    for v in &vs.list {
        tags.push(tag(144, fmt_f64(*v)));
    }
}

/// Add a new dynamic block parameter (and its grip objects) to `block_name` (use `"*Model_Space"`
/// inside `BEDIT`). Returns the newly assigned graph `node` ID.
pub fn add_parameter(d: &mut Drawing, block_name: &str, spec: AuthorParamSpec) -> Result<i32, String> {
    if matches!(&spec, AuthorParamSpec::Visibility { .. })
        && let Some(m) = dynamic_block(d, block_name)
        && m.parameters.iter().any(|p| matches!(p.kind, ParameterKind::Visibility { .. }))
    {
        return Err("A block can have at most one Visibility parameter.".into());
    }
    let (xd_h, gr_h) = ensure_graph(d, block_name);
    let members = read_graph_members(&d.foreign, gr_h);
    let p_node = next_node_id(&members);
    let p_handle = d.new_handle();
    let objs = build_parameter_objects(d, gr_h, p_node, p_handle, block_name, spec)?;
    append_graph_members(d, xd_h, gr_h, objs);
    Ok(p_node)
}

fn build_parameter_objects(
    d: &mut Drawing,
    gr_h: Handle,
    p_node: i32,
    p_handle: Handle,
    block_name: &str,
    spec: AuthorParamSpec,
) -> Result<Vec<(i32, ForeignObject)>, String> {
    match spec {
        AuthorParamSpec::Point { at, label, description, show_properties } => {
            let lbl = non_empty(&label, "Position1");
            let g_node = p_node + 1;
            let g_obj = grip_object(d.new_handle(), gr_h, g_node, "BLOCKPOINTGRIP", at);
            let mut tags = eval_header(p_handle, gr_h, p_node, &lbl);
            tags.extend([
                tag(100, "AcDbBlockParameter"),
                tag(280, if show_properties { "1" } else { "0" }),
                tag(281, "0"),
                tag(100, "AcDbBlock1PtParameter"),
            ]);
            push_pt(&mut tags, 1010, at);
            tags.extend([
                tag(93, g_node.to_string()),
                tag(170, "1"),
                tag(171, "0"),
                tag(100, "AcDbBlockPointParameter"),
                tag(303, &lbl),
                tag(304, &description),
                tag(1011, fmt_f64(at.x)),
                tag(1021, fmt_f64(at.y)),
                tag(1031, fmt_f64(at.z)),
            ]);
            Ok(vec![(p_node, ForeignObject { handle: p_handle, kind: "BLOCKPOINTPARAMETER".into(), tags }), (g_node, g_obj)])
        }
        AuthorParamSpec::Linear { base, end, label, description, values, from_midpoint, show_properties } => {
            if (end - base).len() < 1e-9 {
                return Err("Linear parameter start and end points must be distinct.".into());
            }
            let lbl = non_empty(&label, "Distance1");
            let (g1, g2) = (p_node + 1, p_node + 2);
            let go1 = grip_object(d.new_handle(), gr_h, g1, "BLOCKLINEARGRIP", base);
            let go2 = grip_object(d.new_handle(), gr_h, g2, "BLOCKLINEARGRIP", end);
            let mut tags = eval_header(p_handle, gr_h, p_node, &lbl);
            push_two_pt_parameter(&mut tags, show_properties, base, end, &[g1, g2], from_midpoint);
            tags.extend([tag(100, "AcDbBlockLinearParameter"), tag(305, &lbl), tag(306, &description), tag(140, fmt_f64((end - base).len()))]);
            push_value_set(&mut tags, &values);
            Ok(vec![(p_node, ForeignObject { handle: p_handle, kind: "BLOCKLINEARPARAMETER".into(), tags }), (g1, go1), (g2, go2)])
        }
        AuthorParamSpec::Polar { base, end, label, description, show_properties } => build_two_pt_variant(
            d,
            gr_h,
            (p_node, p_handle),
            ("BLOCKPOLARPARAMETER", "BLOCKPOLARGRIP", "Polar1"),
            (base, end, &label, &description, show_properties),
        ),
        AuthorParamSpec::Xy { base, end, label, description, show_properties } => build_two_pt_variant(
            d,
            gr_h,
            (p_node, p_handle),
            ("BLOCKXYPARAMETER", "BLOCKXYGRIP", "XY1"),
            (base, end, &label, &description, show_properties),
        ),
        AuthorParamSpec::Rotation { base, end, label, description, show_properties } => build_two_pt_variant(
            d,
            gr_h,
            (p_node, p_handle),
            ("BLOCKROTATIONPARAMETER", "BLOCKROTATIONGRIP", "Angle1"),
            (base, end, &label, &description, show_properties),
        ),
        AuthorParamSpec::Flip { base, end, label, description, show_properties } => {
            if (end - base).len() < 1e-9 {
                return Err("Flip reflection line points must be distinct.".into());
            }
            let lbl = non_empty(&label, "Flip state1");
            let g1 = p_node + 1;
            let mid = (base + end) * 0.5;
            let go1 = grip_object(d.new_handle(), gr_h, g1, "BLOCKFLIPGRIP", mid);
            let mut tags = eval_header(p_handle, gr_h, p_node, &lbl);
            push_two_pt_parameter(&mut tags, show_properties, base, end, &[g1], false);
            tags.extend([tag(100, "AcDbBlockFlipParameter"), tag(305, &lbl), tag(306, &description), tag(307, "Not flipped"), tag(308, "Flipped")]);
            Ok(vec![(p_node, ForeignObject { handle: p_handle, kind: "BLOCKFLIPPARAMETER".into(), tags }), (g1, go1)])
        }
        AuthorParamSpec::Visibility { at, label, description, states, show_properties } => {
            let lbl = non_empty(&label, "Visibility1");
            let g1 = p_node + 1;
            let go1 = grip_object(d.new_handle(), gr_h, g1, "BLOCKVISIBILITYGRIP", at);
            let existing_ents = block_entities_handles(d, block_name);
            let existing_members: Vec<Handle> = read_graph_members(&d.foreign, gr_h).into_iter().map(|(_, h)| h).collect();
            let names: Vec<String> =
                if states.is_empty() { vec!["VisibilityState0".to_string()] } else { states.into_iter().filter(|s| !s.trim().is_empty()).collect() };
            let v_states: Vec<VisibilityState> =
                names.into_iter().map(|name| VisibilityState { name, visible: existing_ents.clone(), active: existing_members.clone() }).collect();
            let tags =
                build_visibility_tags((p_handle, gr_h, p_node, g1), (&lbl, &lbl, &description, show_properties), at, &existing_ents, &v_states);
            Ok(vec![(p_node, ForeignObject { handle: p_handle, kind: "BLOCKVISIBILITYPARAMETER".into(), tags }), (g1, go1)])
        }
        AuthorParamSpec::Lookup { at, label, description, show_properties } => {
            let lbl = non_empty(&label, "Lookup1");
            let g1 = p_node + 1;
            let go1 = grip_object(d.new_handle(), gr_h, g1, "BLOCKLOOKUPGRIP", at);
            let mut tags = eval_header(p_handle, gr_h, p_node, &lbl);
            tags.extend([
                tag(100, "AcDbBlockParameter"),
                tag(280, if show_properties { "1" } else { "0" }),
                tag(281, "0"),
                tag(100, "AcDbBlock1PtParameter"),
            ]);
            push_pt(&mut tags, 1010, at);
            tags.extend([
                tag(93, g1.to_string()),
                tag(170, "1"),
                tag(171, "0"),
                tag(100, "AcDbBlockLookupParameter"),
                tag(94, "0"),
                tag(303, &lbl),
                tag(304, &description),
            ]);
            Ok(vec![(p_node, ForeignObject { handle: p_handle, kind: "BLOCKLOOKUPPARAMETER".into(), tags }), (g1, go1)])
        }
    }
}

fn build_two_pt_variant(
    d: &mut Drawing,
    gr_h: Handle,
    ids: (i32, Handle),
    kinds: (&str, &str, &str),
    geom: (Vec3, Vec3, &str, &str, bool),
) -> Result<Vec<(i32, ForeignObject)>, String> {
    let (p_node, p_handle) = ids;
    let (param_kind, grip_kind, fallback_label) = kinds;
    let (base, end, label, description, show_properties) = geom;
    if (end - base).len() < 1e-9 {
        return Err("Parameter base and end points must be distinct.".into());
    }
    let lbl = non_empty(label, fallback_label);
    let (g1, g2) = (p_node + 1, p_node + 2);
    let go1 = grip_object(d.new_handle(), gr_h, g1, grip_kind, base);
    let go2 = grip_object(d.new_handle(), gr_h, g2, grip_kind, end);
    let mut tags = eval_header(p_handle, gr_h, p_node, &lbl);
    push_two_pt_parameter(&mut tags, show_properties, base, end, &[g1, g2], false);
    let subclass = match param_kind {
        "BLOCKPOLARPARAMETER" => "AcDbBlockPolarParameter",
        "BLOCKXYPARAMETER" => "AcDbBlockXYParameter",
        _ => "AcDbBlockRotationParameter",
    };
    tags.extend([tag(100, subclass), tag(305, &lbl), tag(306, description)]);
    Ok(vec![(p_node, ForeignObject { handle: p_handle, kind: param_kind.into(), tags }), (g1, go1), (g2, go2)])
}

fn non_empty(s: &str, fallback: &str) -> String {
    let t = s.trim();
    if t.is_empty() { fallback.to_string() } else { t.to_string() }
}

fn block_entities_handles(d: &Drawing, block_name: &str) -> Vec<Handle> {
    if block_name.eq_ignore_ascii_case("*Model_Space") {
        d.model.handles()
    } else {
        d.block(block_name).map(|b| b.entities.handles()).unwrap_or_default()
    }
}

/// Add a dynamic block action linked to an existing parameter in `block_name`.
/// Returns the newly assigned graph `node` ID.
pub fn add_action(d: &mut Drawing, block_name: &str, spec: AuthorActionSpec) -> Result<i32, String> {
    let m = dynamic_block(d, block_name).ok_or("Block has no dynamic parameters yet; add a parameter first.")?;
    let (xd_h, gr_h) = ensure_graph(d, block_name);
    let members = read_graph_members(&d.foreign, gr_h);
    let a_node = next_node_id(&members);
    let a_handle = d.new_handle();
    let (kind, tags, lookup_param_h) = build_action_object(&m, gr_h, a_node, a_handle, spec)?;
    append_graph_members(d, xd_h, gr_h, vec![(a_node, ForeignObject { handle: a_handle, kind, tags })]);
    if let Some(lp_h) = lookup_param_h {
        let f = Arc::make_mut(&mut d.foreign);
        if let Some(po) = f.objects.iter_mut().find(|o| o.handle == lp_h)
            && let Some(t) = po.tags.iter_mut().find(|t| t.code == 94)
        {
            t.value = a_node.to_string();
        }
    }
    Ok(a_node)
}

fn push_action_base(tags: &mut Vec<RawTag>, selection: &[Handle], at: Vec3) {
    tags.extend([tag(100, "AcDbBlockAction"), tag(70, "0"), tag(71, selection.len().to_string())]);
    for h in selection {
        tags.push(tag(330, h.hex()));
    }
    push_pt(tags, 1010, at);
}

fn conn_names_for_delta(param: &Parameter, end: GripPart) -> (&'static str, &'static str) {
    match (&param.kind, end) {
        (ParameterKind::Point { .. }, _) => ("XDelta", "YDelta"),
        (_, GripPart::Base) => ("BaseXDelta", "BaseYDelta"),
        _ => ("EndXDelta", "EndYDelta"),
    }
}

fn param_base_pt(param: &Parameter) -> Vec3 {
    match &param.kind {
        ParameterKind::Linear { base, .. }
        | ParameterKind::Polar { base, .. }
        | ParameterKind::Xy { base, .. }
        | ParameterKind::Rotation { base, .. }
        | ParameterKind::Flip { base, .. }
        | ParameterKind::Alignment { base, .. } => *base,
        ParameterKind::Point { at, .. }
        | ParameterKind::Visibility { at, .. }
        | ParameterKind::Lookup { at, .. }
        | ParameterKind::BasePoint { at } => *at,
        ParameterKind::Other { .. } => Vec3::ZERO,
    }
}

fn build_action_object(
    m: &DynamicBlock,
    gr_h: Handle,
    a_node: i32,
    a_handle: Handle,
    spec: AuthorActionSpec,
) -> Result<(String, Vec<RawTag>, Option<Handle>), String> {
    match spec {
        AuthorActionSpec::Move { name, parameter_node, end, selection, at, multiplier, angle_offset } => {
            let p = find_param(m, parameter_node)?;
            if selection.is_empty() {
                return Err("Move action requires at least one selected object.".into());
            }
            let nm = non_empty(&name, "Move");
            let (cx, cy) = conn_names_for_delta(p, end);
            let mut tags = eval_header(a_handle, gr_h, a_node, &nm);
            push_action_base(&mut tags, &selection, at);
            tags.extend([
                tag(100, "AcDbBlockMoveAction"),
                tag(92, parameter_node.to_string()),
                tag(93, parameter_node.to_string()),
                tag(301, cx),
                tag(302, cy),
                tag(140, fmt_f64(if multiplier == 0.0 { 1.0 } else { multiplier })),
                tag(141, fmt_f64(angle_offset)),
                tag(280, "0"),
            ]);
            Ok(("BLOCKMOVEACTION".into(), tags, None))
        }
        AuthorActionSpec::Stretch { name, parameter_node, end, frame, selection, at, multiplier, angle_offset } => {
            let p = find_param(m, parameter_node)?;
            if selection.is_empty() {
                return Err("Stretch action requires at least one selected object.".into());
            }
            if frame.len() < 2 {
                return Err("Stretch action requires a stretch frame (2 corner points or polygon).".into());
            }
            let nm = non_empty(&name, "Stretch");
            let (cx, cy) = conn_names_for_delta(p, end);
            let mut tags = eval_header(a_handle, gr_h, a_node, &nm);
            push_action_base(&mut tags, &selection, at);
            tags.extend([
                tag(100, "AcDbBlockStretchAction"),
                tag(92, parameter_node.to_string()),
                tag(93, parameter_node.to_string()),
                tag(301, cx),
                tag(302, cy),
                tag(72, frame.len().to_string()),
            ]);
            for corner in &frame {
                tags.push(tag(1011, fmt_f64(corner.x)));
                tags.push(tag(1021, fmt_f64(corner.y)));
            }
            tags.extend([
                tag(73, "0"),
                tag(75, "0"),
                tag(140, fmt_f64(if multiplier == 0.0 { 1.0 } else { multiplier })),
                tag(141, fmt_f64(angle_offset)),
                tag(280, "0"),
            ]);
            Ok(("BLOCKSTRETCHACTION".into(), tags, None))
        }
        AuthorActionSpec::Scale { name, parameter_node, selection, at } => {
            let p = find_param(m, parameter_node)?;
            if !matches!(p.kind, ParameterKind::Linear { .. } | ParameterKind::Polar { .. } | ParameterKind::Xy { .. }) {
                return Err("Scale action requires a Linear, Polar, or XY parameter.".into());
            }
            if selection.is_empty() {
                return Err("Scale action requires at least one selected object.".into());
            }
            let nm = non_empty(&name, "Scale");
            let base = param_base_pt(p);
            let mut tags = eval_header(a_handle, gr_h, a_node, &nm);
            push_action_base(&mut tags, &selection, at);
            push_base_pt_subclass(&mut tags, parameter_node, base);
            tags.extend([
                tag(100, "AcDbBlockScaleAction"),
                tag(94, parameter_node.to_string()),
                tag(95, parameter_node.to_string()),
                tag(96, parameter_node.to_string()),
                tag(303, "Scale"),
                tag(304, "XScale"),
                tag(305, "YScale"),
            ]);
            Ok(("BLOCKSCALEACTION".into(), tags, None))
        }
        AuthorActionSpec::Rotate { name, parameter_node, selection, at } => {
            let p = find_param(m, parameter_node)?;
            if !matches!(p.kind, ParameterKind::Rotation { .. } | ParameterKind::Polar { .. }) {
                return Err("Rotate action requires a Rotation or Polar parameter.".into());
            }
            if selection.is_empty() {
                return Err("Rotate action requires at least one selected object.".into());
            }
            let nm = non_empty(&name, "Rotate");
            let base = param_base_pt(p);
            let mut tags = eval_header(a_handle, gr_h, a_node, &nm);
            push_action_base(&mut tags, &selection, at);
            push_base_pt_subclass(&mut tags, parameter_node, base);
            tags.extend([tag(100, "AcDbBlockRotationAction"), tag(94, parameter_node.to_string()), tag(303, "Angle")]);
            Ok(("BLOCKROTATEACTION".into(), tags, None))
        }
        AuthorActionSpec::Flip { name, parameter_node, selection, at } => {
            let p = find_param(m, parameter_node)?;
            if !matches!(p.kind, ParameterKind::Flip { .. }) {
                return Err("Flip action requires a Flip parameter.".into());
            }
            if selection.is_empty() {
                return Err("Flip action requires at least one selected object.".into());
            }
            let nm = non_empty(&name, "Flip");
            let mut tags = eval_header(a_handle, gr_h, a_node, &nm);
            push_action_base(&mut tags, &selection, at);
            tags.extend([tag(100, "AcDbBlockFlipAction"), tag(92, parameter_node.to_string()), tag(301, "FlipState")]);
            Ok(("BLOCKFLIPACTION".into(), tags, None))
        }
        AuthorActionSpec::Lookup { name, parameter_node, columns, rows, at } => {
            let p = find_param(m, parameter_node)?;
            if !matches!(p.kind, ParameterKind::Lookup { .. }) {
                return Err("Lookup action requires a Lookup parameter.".into());
            }
            let nm = non_empty(&name, "Lookup");
            let cols = ensure_lookup_column(parameter_node, columns);
            let tags = build_lookup_action_tags((a_handle, gr_h, a_node), &nm, at, &cols, &rows);
            Ok(("BLOCKLOOKUPACTION".into(), tags, Some(p.handle)))
        }
    }
}

fn push_base_pt_subclass(tags: &mut Vec<RawTag>, parameter_node: i32, base: Vec3) {
    tags.extend([
        tag(100, "AcDbBlockActionWithBasePt"),
        tag(92, parameter_node.to_string()),
        tag(93, parameter_node.to_string()),
        tag(301, "UpdatedBaseX"),
        tag(302, "UpdatedBaseY"),
    ]);
    push_pt(tags, 1011, base);
    tags.push(tag(280, "1"));
    push_pt(tags, 1012, base);
}

fn ensure_lookup_column(lookup_node: i32, mut columns: Vec<LookupColumn>) -> Vec<LookupColumn> {
    if !columns.iter().any(|c| c.lookup && c.node == lookup_node) {
        columns.push(LookupColumn { node: lookup_node, property: "lookupString".into(), lookup: true, unmatched: "Custom".into() });
    }
    columns
}

fn build_lookup_action_tags(ids: (Handle, Handle, i32), name: &str, at: Vec3, columns: &[LookupColumn], rows: &[Vec<String>]) -> Vec<RawTag> {
    let (a_handle, gr_h, a_node) = ids;
    let mut tags = eval_header(a_handle, gr_h, a_node, name);
    push_action_base(&mut tags, &[], at);
    tags.extend([tag(100, "AcDbBlockLookupAction"), tag(92, rows.len().to_string()), tag(93, columns.len().to_string()), tag(301, "")]);
    for r in rows {
        for c_idx in 0..columns.len() {
            let cell = r.get(c_idx).map_or("", String::as_str);
            tags.push(tag(302, cell));
        }
    }
    for col in columns {
        tags.extend([
            tag(303, ""),
            tag(94, col.node.to_string()),
            tag(95, "1"),
            tag(96, "0"),
            tag(282, if col.lookup { "1" } else { "0" }),
            tag(305, if col.unmatched.is_empty() { "Custom" } else { &col.unmatched }),
            tag(281, if col.lookup { "1" } else { "0" }),
            tag(304, &col.property),
        ]);
    }
    tags.push(tag(280, "1"));
    tags
}

fn find_param(m: &DynamicBlock, node: i32) -> Result<&Parameter, String> {
    m.parameters.iter().find(|p| p.node == node).ok_or_else(|| format!("Parameter node {node} was not found in this block."))
}

/// Update (or create) the lookup table for `lookup_param_node` in `block_name`.
pub fn set_lookup_table(
    d: &mut Drawing,
    block_name: &str,
    lookup_param_node: i32,
    columns: Vec<LookupColumn>,
    rows: Vec<Vec<String>>,
) -> Result<i32, String> {
    let m = dynamic_block(d, block_name).ok_or("Block has no dynamic parameters.")?;
    let p = find_param(&m, lookup_param_node)?;
    let ParameterKind::Lookup { at, label, .. } = &p.kind else {
        return Err(format!("Node {lookup_param_node} is not a Lookup parameter."));
    };
    let cols = ensure_lookup_column(lookup_param_node, columns);
    if let Some(act) = m
        .actions
        .iter()
        .find(|a| matches!(&a.kind, ActionKind::Lookup { columns: cs, .. } if cs.iter().any(|c| c.lookup && c.node == lookup_param_node)))
    {
        let (_, gr_h) = ensure_graph(d, block_name);
        let tags = build_lookup_action_tags((act.handle, gr_h, act.node), &act.name, act.at, &cols, &rows);
        let f = Arc::make_mut(&mut d.foreign);
        if let Some(ao) = f.objects.iter_mut().find(|o| o.handle == act.handle) {
            ao.tags = tags;
        }
        return Ok(act.node);
    }
    let name = format!("{label}Action");
    let at_pt = *at + Vec3::new(2.0, -2.0, 0.0);
    add_action(d, block_name, AuthorActionSpec::Lookup { name, parameter_node: lookup_param_node, columns: cols, rows, at: at_pt })
}

/// Apply a [`VisibilityOp`] to `block_name`'s visibility parameter (creating one automatically if
/// `AddState` is called when none exists yet).
pub fn edit_visibility(d: &mut Drawing, block_name: &str, op: VisibilityOp) -> Result<Vec<VisibilityState>, String> {
    if dynamic_block(d, block_name).is_none_or(|m| !m.parameters.iter().any(|p| matches!(p.kind, ParameterKind::Visibility { .. }))) {
        if let VisibilityOp::AddState { name, .. } = &op {
            let at = default_author_point(d, block_name);
            add_parameter(
                d,
                block_name,
                AuthorParamSpec::Visibility {
                    at,
                    label: "Visibility1".into(),
                    description: String::new(),
                    states: vec![name.trim().to_string()],
                    show_properties: true,
                },
            )?;
            let m = dynamic_block(d, block_name).ok_or("Failed to create visibility parameter.")?;
            return m
                .parameters
                .into_iter()
                .find_map(|p| if let ParameterKind::Visibility { states, .. } = p.kind { Some(states) } else { None })
                .ok_or_else(|| "Visibility parameter missing.".into());
        }
        return Err("Block has no Visibility parameter; add one with BPARAMETER or BVSTATE New.".into());
    }

    let m = dynamic_block(d, block_name).ok_or("Block has no dynamic data.")?;
    let vp = m.parameters.iter().find(|p| matches!(p.kind, ParameterKind::Visibility { .. })).ok_or("Block has no Visibility parameter.")?;
    let ParameterKind::Visibility { at, label, description, mut controlled, mut states } = vp.kind.clone() else {
        return Err("Block has no Visibility parameter.".into());
    };
    apply_visibility_op(&mut controlled, &mut states, &m, d, block_name, op)?;

    let (_, gr_h) = ensure_graph(d, block_name);
    let grip_node = vp.grips.first().copied().unwrap_or(0);
    let new_tags = build_visibility_tags(
        (vp.handle, gr_h, vp.node, grip_node),
        (&vp.element, &label, &description, vp.show_properties),
        at,
        &controlled,
        &states,
    );
    let f = Arc::make_mut(&mut d.foreign);
    if let Some(vo) = f.objects.iter_mut().find(|o| o.handle == vp.handle) {
        vo.tags = new_tags;
    }
    sync_default_visibility_on_entities(d, block_name, &controlled, states.first());
    Ok(states)
}

fn apply_visibility_op(
    controlled: &mut Vec<Handle>,
    states: &mut Vec<VisibilityState>,
    m: &DynamicBlock,
    d: &Drawing,
    block_name: &str,
    op: VisibilityOp,
) -> Result<(), String> {
    match op {
        VisibilityOp::AddState { name, copy_from } => {
            let nm = name.trim();
            if nm.is_empty() {
                return Err("Visibility state name cannot be empty.".into());
            }
            if states.iter().any(|s| s.name.eq_ignore_ascii_case(nm)) {
                return Err(format!("Visibility state \"{nm}\" already exists."));
            }
            let src =
                copy_from.as_deref().and_then(|c| states.iter().find(|s| s.name.eq_ignore_ascii_case(c.trim()))).or_else(|| states.first()).cloned();
            let (visible, active) = match src {
                Some(s) => (s.visible, s.active),
                None => {
                    (block_entities_handles(d, block_name), m.parameters.iter().map(|p| p.handle).chain(m.actions.iter().map(|a| a.handle)).collect())
                }
            };
            states.push(VisibilityState { name: nm.to_string(), visible, active });
        }
        VisibilityOp::RenameState { old_name, new_name } => {
            let new_nm = new_name.trim();
            if new_nm.is_empty() {
                return Err("New visibility state name cannot be empty.".into());
            }
            if states.iter().any(|s| s.name.eq_ignore_ascii_case(new_nm) && !s.name.eq_ignore_ascii_case(old_name.trim())) {
                return Err(format!("Visibility state \"{new_nm}\" already exists."));
            }
            let st = states
                .iter_mut()
                .find(|s| s.name.eq_ignore_ascii_case(old_name.trim()))
                .ok_or_else(|| format!("Visibility state \"{old_name}\" not found."))?;
            st.name = new_nm.to_string();
        }
        VisibilityOp::DeleteState { name } => {
            if states.len() <= 1 {
                return Err("Cannot delete the last visibility state.".into());
            }
            let before = states.len();
            states.retain(|s| !s.name.eq_ignore_ascii_case(name.trim()));
            if states.len() == before {
                return Err(format!("Visibility state \"{name}\" not found."));
            }
        }
        VisibilityOp::SetDefault { name } => {
            let idx = states
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(name.trim()))
                .ok_or_else(|| format!("Visibility state \"{name}\" not found."))?;
            if idx > 0 {
                let st = states.remove(idx);
                states.insert(0, st);
            }
        }
        VisibilityOp::SetEntitiesVisibility { state, handles, visible } => {
            let st_idx = states
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(state.trim()))
                .ok_or_else(|| format!("Visibility state \"{state}\" not found."))?;
            for h in &handles {
                if !controlled.contains(h) {
                    controlled.push(*h);
                    // Newly controlled entities remain visible in all OTHER states unless hidden.
                    for (i, other) in states.iter_mut().enumerate() {
                        if i != st_idx && !other.visible.contains(h) {
                            other.visible.push(*h);
                        }
                    }
                }
            }
            if let Some(st) = states.get_mut(st_idx) {
                for h in handles {
                    if visible {
                        if !st.visible.contains(&h) {
                            st.visible.push(h);
                        }
                    } else {
                        st.visible.retain(|x| *x != h);
                    }
                }
            }
        }
    }
    Ok(())
}

fn sync_default_visibility_on_entities(d: &mut Drawing, block_name: &str, controlled: &[Handle], default_state: Option<&VisibilityState>) {
    let Some(st) = default_state else { return };
    let vis_set: HashSet<Handle> = st.visible.iter().copied().collect();
    let ctrl_set: HashSet<Handle> = controlled.iter().copied().collect();
    let apply_store = |store: &mut EntityStore| {
        let hs: Vec<Handle> = store.handles().into_iter().filter(|h| ctrl_set.contains(h)).collect();
        for h in hs {
            let show = vis_set.contains(&h);
            store.modify(h, |e| e.common.visible = show);
        }
    };
    if block_name.eq_ignore_ascii_case("*Model_Space") {
        apply_store(&mut d.model);
    } else if let Some(b) = d.blocks.values_mut().find(|b| b.name.eq_ignore_ascii_case(block_name)) {
        apply_store(&mut Arc::make_mut(b).entities);
    }
}

/// Delete parameter or action `node_id` from `block_name`'s evaluation graph (cascading to a
/// parameter's grips and dependent actions). If no parameters or actions remain, removes the
/// empty `ACAD_ENHANCEDBLOCK` dictionary entry cleanly.
pub fn delete_node(d: &mut Drawing, block_name: &str, node_id: i32) -> Result<(), String> {
    let m = dynamic_block(d, block_name).ok_or("Block has no dynamic parameters or actions.")?;
    let rec_key = ("BLOCK_RECORD".to_string(), block_name.trim().to_ascii_uppercase());
    let Some(xd_h) = d.foreign.records.get(&rec_key).and_then(|r| r.xdict) else {
        return Err("Block has no dynamic graph.".into());
    };
    let Some(gr_h) = find_graph_in_dict(&d.foreign, xd_h) else {
        return Err("Block has no dynamic graph.".into());
    };
    let grip_map: HashMap<i32, Handle> = m.grip_objects.iter().copied().collect();
    let mut drop_handles: HashSet<Handle> = HashSet::new();

    if let Some(p) = m.parameters.iter().find(|p| p.node == node_id) {
        drop_handles.insert(p.handle);
        for gid in &p.grips {
            if let Some(gh) = grip_map.get(gid) {
                drop_handles.insert(*gh);
            }
        }
        for a in &m.actions {
            let follows = a.connections.iter().any(|(n, _)| *n == node_id)
                || matches!(&a.kind, ActionKind::Lookup { columns, .. } if columns.iter().any(|c| c.node == node_id));
            if follows {
                drop_handles.insert(a.handle);
            }
        }
    } else if let Some(a) = m.actions.iter().find(|a| a.node == node_id) {
        drop_handles.insert(a.handle);
    } else {
        return Err(format!("Node {node_id} was not found in block \"{block_name}\"."));
    }

    let remaining: Vec<(i32, Handle)> = read_graph_members(&d.foreign, gr_h).into_iter().filter(|(_, h)| !drop_handles.contains(h)).collect();

    let has_primary = remaining.iter().any(|(_, h)| {
        d.foreign
            .objects
            .iter()
            .find(|o| o.handle == *h)
            .is_some_and(|o| o.kind.to_ascii_uppercase().ends_with("PARAMETER") || o.kind.to_ascii_uppercase().ends_with("ACTION"))
    });

    if !has_primary {
        remove_block_graph(d, block_name);
        return Ok(());
    }

    let f = Arc::make_mut(&mut d.foreign);
    f.objects.retain(|o| !drop_handles.contains(&o.handle));
    if let Some(g) = f.objects.iter_mut().find(|o| o.handle == gr_h) {
        g.tags = build_graph_tags(gr_h, xd_h, &remaining);
    }
    Ok(())
}

/// Remove `block_name`'s `ACAD_ENHANCEDBLOCK` evaluation graph and its member objects from `d`.
pub fn remove_block_graph(d: &mut Drawing, block_name: &str) {
    let rec_key = ("BLOCK_RECORD".to_string(), block_name.trim().to_ascii_uppercase());
    let Some(xd_h) = d.foreign.records.get(&rec_key).and_then(|r| r.xdict) else { return };
    let Some(gr_h) = find_graph_in_dict(&d.foreign, xd_h) else { return };
    let members: HashSet<Handle> = read_graph_members(&d.foreign, gr_h).into_iter().map(|(_, h)| h).collect();

    let f = Arc::make_mut(&mut d.foreign);
    f.objects.retain(|o| o.handle != gr_h && !members.contains(&o.handle));
    let mut dict_empty = false;
    if let Some(dict) = f.objects.iter_mut().find(|o| o.handle == xd_h) {
        let mut cleaned = Vec::with_capacity(dict.tags.len());
        let mut i = 0;
        while let Some(t) = dict.tags.get(i) {
            if t.code == 3 && t.value.eq_ignore_ascii_case("ACAD_ENHANCEDBLOCK") && dict.tags.get(i + 1).is_some_and(|n| matches!(n.code, 350 | 360))
            {
                i += 2;
                continue;
            }
            cleaned.push(t.clone());
            i += 1;
        }
        dict_empty = !cleaned.iter().any(|t| t.code == 3);
        dict.tags = cleaned;
    }
    if dict_empty {
        f.objects.retain(|o| o.handle != xd_h);
        if let Some(rec) = f.records.get_mut(&rec_key) {
            rec.xdict = None;
            if rec.is_empty() {
                f.records.remove(&rec_key);
            }
        }
    }
}

/// Default placement point above a block's geometry bounds for UI/one-point authoring.
pub fn default_author_point(d: &Drawing, block_name: &str) -> Vec3 {
    let store = if block_name.eq_ignore_ascii_case("*Model_Space") { Some(&d.model) } else { d.block(block_name).map(|b| &b.entities) };
    let mut b = Bounds2::EMPTY;
    if let Some(st) = store {
        for e in st.iter() {
            for pt in e.kind.grips() {
                b = b.union(&Bounds2::new(pt, pt));
            }
        }
    }
    if b.is_empty() {
        Vec3::new(0.0, 10.0, 0.0)
    } else {
        let pad = b.height().max(b.width()).max(10.0) * 0.15;
        Vec3::new(b.min.x, b.max.y + pad, 0.0)
    }
}

/// Copy `src_block`'s `ACAD_ENHANCEDBLOCK` evaluation graph onto `dst_block` (e.g. `"*Model_Space"`
/// when opening `BEDIT`, or back onto `target_block` on `BSAVE`), translating parameter/action
/// coordinates by `delta`.
pub fn copy_block_graph(src: &Drawing, src_block: &str, dst: &mut Drawing, dst_block: &str, delta: Vec2) {
    remove_block_graph(dst, dst_block);
    let src_key = ("BLOCK_RECORD".to_string(), src_block.trim().to_ascii_uppercase());
    let Some(src_xd) = src.foreign.records.get(&src_key).and_then(|r| r.xdict) else { return };
    let Some(src_gr) = find_graph_in_dict(&src.foreign, src_xd) else { return };
    let src_members = read_graph_members(&src.foreign, src_gr);
    if src_members.is_empty() {
        return;
    }
    let src_by_h: HashMap<Handle, &ForeignObject> = src.foreign.objects.iter().map(|o| (o.handle, o)).collect();
    let (dst_xd, dst_gr) = ensure_graph(dst, dst_block);

    let mut handle_map: HashMap<Handle, Handle> = HashMap::new();
    handle_map.insert(src_gr, dst_gr);
    for (_, old_h) in &src_members {
        handle_map.insert(*old_h, dst.new_handle());
    }

    let mut new_members: Vec<(i32, Handle)> = Vec::with_capacity(src_members.len());
    let mut new_objects: Vec<ForeignObject> = Vec::with_capacity(src_members.len());
    for (node, old_h) in &src_members {
        let Some(orig) = src_by_h.get(old_h) else { continue };
        let Some(nh) = handle_map.get(old_h).copied() else { continue };
        let mut tags = remap_member_tags(&orig.tags, &handle_map, nh, dst_gr);
        if delta != Vec2::ZERO {
            shift_member_coords(&orig.kind, &mut tags, delta);
        }
        new_members.push((*node, nh));
        new_objects.push(ForeignObject { handle: nh, kind: orig.kind.clone(), tags });
    }

    let f = Arc::make_mut(&mut dst.foreign);
    f.objects.extend(new_objects);
    if let Some(g) = f.objects.iter_mut().find(|o| o.handle == dst_gr) {
        g.tags = build_graph_tags(dst_gr, dst_xd, &new_members);
    }
}

fn remap_member_tags(tags: &[RawTag], handle_map: &HashMap<Handle, Handle>, self_h: Handle, graph_h: Handle) -> Vec<RawTag> {
    let mut out = Vec::with_capacity(tags.len());
    let mut seen_subclass = false;
    for t in tags {
        if t.code == 100 {
            seen_subclass = true;
        }
        if t.code == 5 && !seen_subclass {
            out.push(tag(5, self_h.hex()));
            continue;
        }
        if t.code == 330 && !seen_subclass {
            out.push(tag(330, graph_h.hex()));
            continue;
        }
        if matches!(t.code, 330..=369)
            && let Some(oh) = Handle::parse_hex(&t.value)
            && let Some(nh) = handle_map.get(&oh)
        {
            out.push(tag(t.code, nh.hex()));
            continue;
        }
        out.push(t.clone());
    }
    out
}

fn shift_member_coords(kind: &str, tags: &mut [RawTag], delta: Vec2) {
    let mut sec = "";
    for t in tags.iter_mut() {
        if t.code == 100 {
            sec = t.value.as_str();
            continue;
        }
        let shift_1010 = sec.eq_ignore_ascii_case("AcDbBlock1PtParameter")
            || sec.eq_ignore_ascii_case("AcDbBlock2PtParameter")
            || sec.eq_ignore_ascii_case("AcDbBlockAction")
            || sec.eq_ignore_ascii_case("AcDbBlockGrip");
        let shift_1011 = sec.eq_ignore_ascii_case("AcDbBlock2PtParameter")
            || sec.eq_ignore_ascii_case("AcDbBlockPointParameter")
            || sec.eq_ignore_ascii_case("AcDbBlockStretchAction")
            || sec.eq_ignore_ascii_case("AcDbBlockActionWithBasePt");
        let shift_1012 = sec.eq_ignore_ascii_case("AcDbBlockActionWithBasePt")
            || kind.eq_ignore_ascii_case("BLOCKSCALEACTION")
            || kind.eq_ignore_ascii_case("BLOCKROTATEACTION");
        let off = match t.code {
            1010 if shift_1010 => Some(delta.x),
            1020 if shift_1010 => Some(delta.y),
            1011 if shift_1011 => Some(delta.x),
            1021 if shift_1011 => Some(delta.y),
            1012 if shift_1012 => Some(delta.x),
            1022 if shift_1012 => Some(delta.y),
            _ => None,
        };
        if let Some(d) = off
            && let Ok(v) = t.value.trim().parse::<f64>()
            && v.is_finite()
        {
            t.value = fmt_f64(v + d);
        }
    }
}

/// Stamp `AcDbBlockRepETag` xdata on any entity of `block_name` that doesn't already carry one,
/// and sync default visibility state flags onto its entities so new and existing `INSERT`
/// references pair reliably during dynamic evaluation and DXF round-trip.
pub fn finalize_dynamic_block(d: &mut Drawing, block_name: &str) {
    let Some(m) = dynamic_block(d, block_name) else { return };
    if let Some(b) = d.blocks.values_mut().find(|b| b.name.eq_ignore_ascii_case(block_name)) {
        let blk = Arc::make_mut(b);
        let hs = blk.entities.handles();
        for (idx, h) in hs.into_iter().enumerate() {
            blk.entities.modify(h, |e| {
                let has_etag = e.common.xdata.iter().any(|t| t.code == 1001 && t.value.eq_ignore_ascii_case(ETAG_APP));
                if !has_etag {
                    e.common.xdata.push(tag(1001, ETAG_APP));
                    e.common.xdata.push(tag(1071, (idx as i64 + 1).to_string()));
                }
            });
        }
    }
    for p in &m.parameters {
        if let ParameterKind::Visibility { controlled, states, .. } = &p.kind {
            sync_default_visibility_on_entities(d, block_name, controlled, states.first());
        }
    }
}
