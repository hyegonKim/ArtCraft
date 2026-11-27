//! Dynamic block authoring commands inside the Block Editor (`BEDIT`):
//! - `BPARAMETER` (`bparameter`, `bparam`, `-bparameter`): add/list/delete Point, Linear, Polar,
//!   XY, Rotation, Flip, Visibility, and Lookup parameters,
//! - `BACTION` (`baction`, `bact`, `-baction`, `bactiontool`): add/delete Move, Stretch, Scale,
//!   Rotate, Flip, and Lookup actions,
//! - `BVSTATE` (`bvstate`, `bvisibility`, `-bvstate`): manage visibility states and per-state
//!   entity visibility (`new`, `rename`, `delete`, `default`/`current`, `show`, `hide`, `list`),
//! - `BLOOKUP` (`blookup`, `-blookup`): define or update lookup tables for Lookup parameters.

use cadcraft_doc::Handle;
use cadcraft_doc::dynblock::author::{
    AuthorActionSpec, AuthorParamSpec, VisibilityOp, add_action, add_parameter, default_author_point, delete_node, edit_visibility, set_lookup_table,
};
use cadcraft_doc::dynblock::{DynamicBlock, GripPart, LookupColumn, ParameterKind, ValueSet, dynamic_block};
use cadcraft_geom::{Vec2, Vec3};
use serde_json::{Value, json};

use crate::cmd::curves::{KW, handle_value};
use crate::cmd::helpers::v3;
use crate::cmd::machines::{SelOutcome, SelectPhase};
use crate::cmd::{CommandSpec, bad, bool_or, f64_or, point_param, str_param};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

const MODEL_GRAPH: &str = "*Model_Space";

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("bparameter", "Dynamic Parameter", run_bparameter)
            .alias(&["bparam", "-bparameter"])
            .params("{action?: \"add\"|\"delete\"|\"list\", type?: \"point\"|\"linear\"|\"distance\"|\"polar\"|\"xy\"|\"rotation\"|\"flip\"|\"visibility\"|\"lookup\", label?, description?, at?: [x,y], base?: [x,y], end?: [x,y], values?: [number], minimum?, maximum?, increment?, fromMidpoint?, states?: [string], showProperties?: bool, node?: integer}")
            .enabled(in_editor)
            .interactive(|_| Ok(Box::new(BparamM::default()))),
        CommandSpec::new("baction", "Dynamic Action", run_baction)
            .alias(&["bact", "-baction", "bactiontool"])
            .params("{action?: \"add\"|\"delete\", type?: \"move\"|\"stretch\"|\"scale\"|\"rotate\"|\"flip\"|\"lookup\", parameter?: node|label, name?, handles?: [handle], end?: \"base\"|\"end\", frame?: [[x,y], ...], at?: [x,y], multiplier?, angleOffset?, columns?, rows?, node?: integer}")
            .enabled(in_editor)
            .interactive(|_| Ok(Box::new(BactionM::default()))),
        CommandSpec::new("bvstate", "Visibility States", run_bvstate)
            .alias(&["bvisibility", "-bvstate"])
            .params("{op?: \"new\"|\"rename\"|\"delete\"|\"default\"|\"current\"|\"show\"|\"hide\"|\"list\", name?, newName?, copyFrom?, state?, handles?: [handle]}")
            .enabled(in_editor)
            .interactive(|_| Ok(Box::new(BvstateM::default()))),
        CommandSpec::new("blookup", "Lookup Table", run_blookup)
            .alias(&["-blookup", "blookuptable"])
            .params("{parameter: node|label, columns: [{node?, parameter?, property?, lookup?, unmatched?}], rows: [[string, ...]]}")
            .enabled(in_editor),
    ]
}

fn in_editor(s: &Session) -> std::result::Result<(), String> {
    match s.state() {
        Ok(st) if st.block_edit.is_some() => Ok(()),
        Ok(_) => Err("Open a block in the Block Editor (BEDIT) first.".into()),
        Err(e) => Err(e.to_string()),
    }
}

fn editor_block_label(s: &Session) -> String {
    s.state().ok().and_then(|st| st.block_editor().map(str::to_string)).unwrap_or_else(|| MODEL_GRAPH.to_string())
}

fn graph_json(s: &Session) -> Value {
    s.doc().ok().and_then(|d| dynamic_block(d, MODEL_GRAPH)).and_then(|m| serde_json::to_value(m).ok()).unwrap_or(Value::Null)
}

fn parse_handles(p: &Value, key: &str, fallback_sel: &[Handle]) -> Result<Vec<Handle>> {
    match p.get(key) {
        Some(Value::Array(arr)) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                let h = handle_value(item).ok_or_else(|| bad("baction", format!("invalid handle in `{key}`: {item}")))?;
                out.push(h);
            }
            Ok(out)
        }
        None => Ok(fallback_sel.to_vec()),
        Some(other) => Err(bad("baction", format!("`{key}` must be an array of handles, not {other}"))),
    }
}

fn resolve_param_node(m: &DynamicBlock, v: &Value, cmd: &str) -> Result<i32> {
    if let Some(n) = v.as_i64().and_then(|x| i32::try_from(x).ok()) {
        if m.parameters.iter().any(|p| p.node == n) {
            return Ok(n);
        }
        return Err(bad(cmd, format!("parameter node {n} not found")));
    }
    if let Some(s) = v.as_str() {
        let t = s.trim();
        if let Ok(n) = t.parse::<i32>()
            && m.parameters.iter().any(|p| p.node == n)
        {
            return Ok(n);
        }
        if let Some(p) = m.parameters.iter().find(|p| p.display_label().eq_ignore_ascii_case(t)) {
            return Ok(p.node);
        }
        return Err(bad(cmd, format!("parameter \"{t}\" not found")));
    }
    Err(bad(cmd, "`parameter` must be a parameter node ID or parameter label"))
}

fn parse_value_set(p: &Value) -> ValueSet {
    let mut flags = 0i32;
    let minimum = p.get("minimum").and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(0.0);
    if p.get("minimum").and_then(Value::as_f64).is_some_and(f64::is_finite) {
        flags |= 1;
    }
    let maximum = p.get("maximum").and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(0.0);
    if p.get("maximum").and_then(Value::as_f64).is_some_and(f64::is_finite) {
        flags |= 2;
    }
    let increment = p.get("increment").and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0).unwrap_or(0.0);
    if increment > 0.0 {
        flags |= 4;
    }
    let list: Vec<f64> = p
        .get("values")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_f64).filter(|x| x.is_finite()).collect())
        .unwrap_or_default();
    if !list.is_empty() {
        flags |= 8;
    }
    ValueSet { flags, minimum, maximum, increment, list }
}

fn next_default_label(d: &cadcraft_doc::Drawing, prefix: &str) -> String {
    let existing: Vec<String> =
        dynamic_block(d, MODEL_GRAPH).map(|m| m.parameters.iter().map(|p| p.display_label().to_ascii_uppercase()).collect()).unwrap_or_default();
    for idx in 1..=999 {
        let cand = format!("{prefix}{idx}");
        if !existing.contains(&cand.to_ascii_uppercase()) {
            return cand;
        }
    }
    format!("{prefix}1")
}

fn run_bparameter(s: &mut Session, p: &Value) -> Result<Value> {
    let act = str_param(p, "action").unwrap_or("add").trim().to_ascii_lowercase();
    let blk = editor_block_label(s);
    match act.as_str() {
        "list" => Ok(json!({ "block": blk, "dynamic": graph_json(s) })),
        "delete" | "remove" => {
            let node = p
                .get("node")
                .and_then(Value::as_i64)
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| bad("bparameter", "`node` is required for delete"))?;
            delete_node(s.doc_mut()?, MODEL_GRAPH, node).map_err(|e| bad("bparameter", e))?;
            Ok(json!({ "block": blk, "deleted": node, "dynamic": graph_json(s) }))
        }
        "add" => {
            let spec = build_param_spec(s.doc()?, p)?;
            let node = add_parameter(s.doc_mut()?, MODEL_GRAPH, spec).map_err(|e| bad("bparameter", e))?;
            Ok(json!({ "block": blk, "node": node, "dynamic": graph_json(s) }))
        }
        other => Err(bad("bparameter", format!("unknown action {other:?}; expected add, delete, or list"))),
    }
}

fn build_param_spec(d: &cadcraft_doc::Drawing, p: &Value) -> Result<AuthorParamSpec> {
    let kind = str_param(p, "type").unwrap_or("linear").trim().to_ascii_lowercase();
    let description = str_param(p, "description").unwrap_or("").to_string();
    let show_properties = bool_or(p, "showProperties", true);
    let default_pt = default_author_point(d, MODEL_GRAPH);
    let at_pt = || point_param(p, "at").map(v3).unwrap_or(default_pt);
    let two_pts = || -> (Vec3, Vec3) {
        let b = point_param(p, "base").map(v3).unwrap_or(Vec3::ZERO);
        let e = point_param(p, "end").map(v3).unwrap_or_else(|| b + Vec3::new(10.0, 0.0, 0.0));
        (b, e)
    };
    let label_or = |prefix: &str| {
        str_param(p, "label").map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| next_default_label(d, prefix))
    };

    match kind.as_str() {
        "point" | "position" => Ok(AuthorParamSpec::Point { at: at_pt(), label: label_or("Position"), description, show_properties }),
        "linear" | "distance" => {
            let (base, end) = two_pts();
            Ok(AuthorParamSpec::Linear {
                base,
                end,
                label: label_or("Distance"),
                description,
                values: parse_value_set(p),
                from_midpoint: bool_or(p, "fromMidpoint", false),
                show_properties,
            })
        }
        "polar" => {
            let (base, end) = two_pts();
            Ok(AuthorParamSpec::Polar { base, end, label: label_or("Polar"), description, show_properties })
        }
        "xy" => {
            let (base, end) = two_pts();
            Ok(AuthorParamSpec::Xy { base, end, label: label_or("XY"), description, show_properties })
        }
        "rotation" | "angle" => {
            let (base, end) = two_pts();
            Ok(AuthorParamSpec::Rotation { base, end, label: label_or("Angle"), description, show_properties })
        }
        "flip" => {
            let (base, end) = match (point_param(p, "base"), point_param(p, "end")) {
                (Some(b), Some(e)) => (v3(b), v3(e)),
                (Some(b), None) => (v3(b), v3(b) + Vec3::new(0.0, 10.0, 0.0)),
                _ => (Vec3::ZERO, Vec3::new(0.0, 10.0, 0.0)),
            };
            Ok(AuthorParamSpec::Flip { base, end, label: label_or("Flip state"), description, show_properties })
        }
        "visibility" => {
            let states = p
                .get("states")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_else(|| vec!["VisibilityState0".to_string()]);
            Ok(AuthorParamSpec::Visibility { at: at_pt(), label: label_or("Visibility"), description, states, show_properties })
        }
        "lookup" => Ok(AuthorParamSpec::Lookup { at: at_pt(), label: label_or("Lookup"), description, show_properties }),
        other => Err(bad("bparameter", format!("unknown parameter type {other:?}"))),
    }
}

fn run_baction(s: &mut Session, p: &Value) -> Result<Value> {
    let op = str_param(p, "action").unwrap_or("add").trim().to_ascii_lowercase();
    let blk = editor_block_label(s);
    if matches!(op.as_str(), "delete" | "remove") {
        let node = p
            .get("node")
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| bad("baction", "`node` is required for delete"))?;
        delete_node(s.doc_mut()?, MODEL_GRAPH, node).map_err(|e| bad("baction", e))?;
        return Ok(json!({ "block": blk, "deleted": node, "dynamic": graph_json(s) }));
    }
    let m = dynamic_block(s.doc()?, MODEL_GRAPH).ok_or_else(|| bad("baction", "block has no dynamic parameters yet; run BPARAMETER first"))?;
    let param_val = p.get("parameter").ok_or_else(|| bad("baction", "`parameter` (node ID or label) is required"))?;
    let parameter_node = resolve_param_node(&m, param_val, "baction")?;
    let sel = parse_handles(p, "handles", &s.selection())?;
    let spec = build_action_spec(s.doc()?, &m, parameter_node, sel, p)?;
    let node = add_action(s.doc_mut()?, MODEL_GRAPH, spec).map_err(|e| bad("baction", e))?;
    Ok(json!({ "block": blk, "node": node, "dynamic": graph_json(s) }))
}

fn build_action_spec(
    d: &cadcraft_doc::Drawing,
    m: &DynamicBlock,
    parameter_node: i32,
    selection: Vec<Handle>,
    p: &Value,
) -> Result<AuthorActionSpec> {
    let kind = str_param(p, "type").ok_or_else(|| bad("baction", "`type` (move|stretch|scale|rotate|flip|lookup) is required"))?;
    let name = str_param(p, "name").unwrap_or("").to_string();
    let at = point_param(p, "at").map(v3).unwrap_or_else(|| default_author_point(d, MODEL_GRAPH));
    let end = match str_param(p, "end").unwrap_or("end").trim().to_ascii_lowercase().as_str() {
        "base" | "start" => GripPart::Base,
        "at" => GripPart::At,
        _ => GripPart::End,
    };
    let multiplier = f64_or(p, "multiplier", 1.0);
    let angle_offset = f64_or(p, "angleOffset", 0.0).to_radians();

    match kind.trim().to_ascii_lowercase().as_str() {
        "move" => Ok(AuthorActionSpec::Move { name, parameter_node, end, selection, at, multiplier, angle_offset }),
        "stretch" => {
            let frame = parse_frame(p)?;
            Ok(AuthorActionSpec::Stretch { name, parameter_node, end, frame, selection, at, multiplier, angle_offset })
        }
        "scale" => Ok(AuthorActionSpec::Scale { name, parameter_node, selection, at }),
        "rotate" => Ok(AuthorActionSpec::Rotate { name, parameter_node, selection, at }),
        "flip" => Ok(AuthorActionSpec::Flip { name, parameter_node, selection, at }),
        "lookup" => {
            let (columns, rows) = parse_lookup_payload(m, parameter_node, p, "baction")?;
            Ok(AuthorActionSpec::Lookup { name, parameter_node, columns, rows, at })
        }
        other => Err(bad("baction", format!("unknown action type {other:?}"))),
    }
}

fn parse_frame(p: &Value) -> Result<Vec<Vec2>> {
    let Some(arr) = p.get("frame").and_then(Value::as_array) else {
        return Err(bad("baction", "`frame` (array of 2 corner points [[x1,y1],[x2,y2]]) is required for stretch"));
    };
    let mut pts = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(coords) = item.as_array() else {
            return Err(bad("baction", "each `frame` corner must be [x, y]"));
        };
        let x = coords.first().and_then(Value::as_f64).ok_or_else(|| bad("baction", "invalid frame x"))?;
        let y = coords.get(1).and_then(Value::as_f64).ok_or_else(|| bad("baction", "invalid frame y"))?;
        pts.push(Vec2::new(x, y));
    }
    if pts.len() < 2 {
        return Err(bad("baction", "`frame` requires at least 2 points"));
    }
    Ok(pts)
}

fn parse_lookup_payload(m: &DynamicBlock, lookup_node: i32, p: &Value, cmd: &str) -> Result<(Vec<LookupColumn>, Vec<Vec<String>>)> {
    let mut columns = Vec::new();
    if let Some(cols) = p.get("columns").and_then(Value::as_array) {
        for c in cols {
            let node = if let Some(nv) = c.get("node") {
                resolve_param_node(m, nv, cmd)?
            } else if let Some(pv) = c.get("parameter") {
                resolve_param_node(m, pv, cmd)?
            } else {
                return Err(bad(cmd, "each lookup column requires `node` or `parameter`"));
            };
            let param = m.parameters.iter().find(|x| x.node == node);
            let default_prop = match param.map(|x| &x.kind) {
                Some(ParameterKind::Visibility { .. }) => "VisibilityState",
                Some(ParameterKind::Lookup { .. }) => "lookupString",
                _ => "UpdatedDistance",
            };
            let lookup = c.get("lookup").and_then(Value::as_bool).unwrap_or(node == lookup_node);
            let property = c.get("property").and_then(Value::as_str).unwrap_or(default_prop).to_string();
            let unmatched = c.get("unmatched").and_then(Value::as_str).unwrap_or("Custom").to_string();
            columns.push(LookupColumn { node, property, lookup, unmatched });
        }
    }
    let rows: Vec<Vec<String>> = p
        .get("rows")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(Value::as_array)
                .map(|r| {
                    r.iter()
                        .map(|cell| match cell {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            other => other.to_string(),
                        })
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((columns, rows))
}

fn run_bvstate(s: &mut Session, p: &Value) -> Result<Value> {
    let op_name = str_param(p, "op").unwrap_or("list").trim().to_ascii_lowercase();
    let blk = editor_block_label(s);
    if op_name == "list" {
        let states = dynamic_block(s.doc()?, MODEL_GRAPH)
            .and_then(|m| {
                m.parameters.into_iter().find_map(|x| match x.kind {
                    ParameterKind::Visibility { states, .. } => Some(states),
                    _ => None,
                })
            })
            .unwrap_or_default();
        return Ok(json!({ "block": blk, "states": states }));
    }
    let sel = parse_handles(p, "handles", &s.selection())?;
    let op = match op_name.as_str() {
        "new" | "add" => {
            let name = str_param(p, "name").ok_or_else(|| bad("bvstate", "`name` is required"))?.to_string();
            let copy_from = str_param(p, "copyFrom").map(str::to_string);
            VisibilityOp::AddState { name, copy_from }
        }
        "rename" => {
            let old_name = str_param(p, "name").ok_or_else(|| bad("bvstate", "`name` is required"))?.to_string();
            let new_name = str_param(p, "newName").ok_or_else(|| bad("bvstate", "`newName` is required"))?.to_string();
            VisibilityOp::RenameState { old_name, new_name }
        }
        "delete" | "remove" => {
            let name = str_param(p, "name").ok_or_else(|| bad("bvstate", "`name` is required"))?.to_string();
            VisibilityOp::DeleteState { name }
        }
        "default" | "current" | "set" => {
            let name = str_param(p, "name").or_else(|| str_param(p, "state")).ok_or_else(|| bad("bvstate", "`name` is required"))?.to_string();
            VisibilityOp::SetDefault { name }
        }
        "show" | "hide" => {
            if sel.is_empty() {
                return Err(bad("bvstate", "`handles` (or an active selection) is required for show/hide"));
            }
            let doc = s.doc()?;
            let state = str_param(p, "state")
                .or_else(|| str_param(p, "name"))
                .map(str::to_string)
                .or_else(|| default_vis_state_name(doc))
                .ok_or_else(|| bad("bvstate", "`state` is required"))?;
            VisibilityOp::SetEntitiesVisibility { state, handles: sel, visible: op_name == "show" }
        }
        other => return Err(bad("bvstate", format!("unknown op {other:?}"))),
    };
    let states = edit_visibility(s.doc_mut()?, MODEL_GRAPH, op).map_err(|e| bad("bvstate", e))?;
    Ok(json!({ "block": blk, "states": states, "dynamic": graph_json(s) }))
}

fn default_vis_state_name(d: &cadcraft_doc::Drawing) -> Option<String> {
    dynamic_block(d, MODEL_GRAPH)?.parameters.into_iter().find_map(|p| match p.kind {
        ParameterKind::Visibility { states, .. } => states.first().map(|st| st.name.clone()),
        _ => None,
    })
}

fn run_blookup(s: &mut Session, p: &Value) -> Result<Value> {
    let blk = editor_block_label(s);
    let m = dynamic_block(s.doc()?, MODEL_GRAPH).ok_or_else(|| bad("blookup", "block has no dynamic parameters; add a Lookup parameter first"))?;
    let param_val = p.get("parameter").ok_or_else(|| bad("blookup", "`parameter` (Lookup parameter node or label) is required"))?;
    let lookup_node = resolve_param_node(&m, param_val, "blookup")?;
    let (columns, rows) = parse_lookup_payload(&m, lookup_node, p, "blookup")?;
    let action_node = set_lookup_table(s.doc_mut()?, MODEL_GRAPH, lookup_node, columns, rows).map_err(|e| bad("blookup", e))?;
    Ok(json!({ "block": blk, "actionNode": action_node, "dynamic": graph_json(s) }))
}

// ---------------- Interactive state machines ----------------

#[derive(Default)]
struct BparamM {
    phase: BparamPhase,
    kind: String,
    p1: Option<Vec2>,
    p2: Option<Vec2>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum BparamPhase {
    #[default]
    ChooseType,
    FirstPoint,
    SecondPoint,
    Label,
    DeleteNode,
}

impl Interactive for BparamM {
    fn name(&self) -> &'static str {
        "BPARAMETER"
    }
    fn begin(&mut self, _s: &mut Session) -> Result<Step> {
        Ok(Step::Continue)
    }
    fn prompt(&self, s: &Session) -> Prompt {
        match self.phase {
            BparamPhase::ChooseType => Prompt::new("Enter parameter type [Point/Linear/Polar/XY/Rotation/Flip/Visibility/Lookup/Delete]", KW)
                .kw(&["Point", "Linear", "Polar", "XY", "Rotation", "Flip", "Visibility", "Lookup", "Delete"])
                .default("Linear"),
            BparamPhase::FirstPoint => {
                let one = matches!(self.kind.as_str(), "point" | "visibility" | "lookup");
                Prompt::new(if one { "Specify parameter location" } else { "Specify start point" }, Accept::POINT)
            }
            BparamPhase::SecondPoint => {
                let p = Prompt::new("Specify endpoint", Accept::POINT);
                if let Some(b) = self.p1 { p.base(b) } else { p }
            }
            BparamPhase::Label => {
                let prefix = match self.kind.as_str() {
                    "point" => "Position",
                    "polar" => "Polar",
                    "xy" => "XY",
                    "rotation" => "Angle",
                    "flip" => "Flip state",
                    "visibility" => "Visibility",
                    "lookup" => "Lookup",
                    _ => "Distance",
                };
                let def = s.doc().map(|d| next_default_label(d, prefix)).unwrap_or_else(|_| format!("{prefix}1"));
                Prompt::new("Enter parameter label", Accept::TEXT).default(&def)
            }
            BparamPhase::DeleteNode => Prompt::new("Enter parameter or action node ID to delete", Accept::TEXT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.phase {
            BparamPhase::ChooseType => {
                let kw = match i {
                    Input::Enter => "linear".to_string(),
                    Input::Keyword(k) | Input::Text(k) => k.trim().to_ascii_lowercase(),
                    _ => return Ok(Step::Continue),
                };
                if kw == "delete" || kw == "d" {
                    self.phase = BparamPhase::DeleteNode;
                    return Ok(Step::Continue);
                }
                self.kind = match kw.as_str() {
                    "p" | "point" => "point",
                    "l" | "linear" => "linear",
                    "po" | "polar" => "polar",
                    "xy" => "xy",
                    "r" | "rotation" => "rotation",
                    "f" | "flip" => "flip",
                    "v" | "visibility" => "visibility",
                    "lo" | "lookup" => "lookup",
                    _ => return Err(EngineError::Other(format!("Unknown parameter type \"{kw}\"."))),
                }
                .to_string();
                self.phase = BparamPhase::FirstPoint;
                Ok(Step::Continue)
            }
            BparamPhase::FirstPoint => {
                let pt = match i {
                    Input::Point(pt) => pt,
                    Input::Deferred(d) => d.at,
                    _ => return Ok(Step::Continue),
                };
                self.p1 = Some(pt);
                if matches!(self.kind.as_str(), "point" | "visibility" | "lookup") {
                    self.phase = BparamPhase::Label;
                } else {
                    self.phase = BparamPhase::SecondPoint;
                }
                Ok(Step::Continue)
            }
            BparamPhase::SecondPoint => {
                let pt = match i {
                    Input::Point(pt) => pt,
                    Input::Deferred(d) => d.at,
                    _ => return Ok(Step::Continue),
                };
                self.p2 = Some(pt);
                self.phase = BparamPhase::Label;
                Ok(Step::Continue)
            }
            BparamPhase::Label => {
                let lbl = match i {
                    Input::Enter => String::new(),
                    Input::Text(t) | Input::Keyword(t) => t.trim().to_string(),
                    _ => return Ok(Step::Continue),
                };
                let mut params = json!({ "type": self.kind });
                if !lbl.is_empty()
                    && let Some(o) = params.as_object_mut()
                {
                    o.insert("label".into(), json!(lbl));
                }
                if let Some(p1) = self.p1
                    && let Some(o) = params.as_object_mut()
                {
                    if matches!(self.kind.as_str(), "point" | "visibility" | "lookup") {
                        o.insert("at".into(), json!([p1.x, p1.y]));
                    } else {
                        o.insert("base".into(), json!([p1.x, p1.y]));
                    }
                }
                if let Some(p2) = self.p2
                    && let Some(o) = params.as_object_mut()
                {
                    o.insert("end".into(), json!([p2.x, p2.y]));
                }
                let res = run_bparameter(s, &params)?;
                s.echo(format!("Added parameter node {}.", res["node"]));
                Ok(Step::Done)
            }
            BparamPhase::DeleteNode => {
                let txt = match i {
                    Input::Text(t) | Input::Keyword(t) => t,
                    _ => return Ok(Step::Done),
                };
                let node: i32 = txt.trim().parse().map_err(|_| EngineError::Other("Invalid node ID.".into()))?;
                run_bparameter(s, &json!({ "action": "delete", "node": node }))?;
                s.echo(format!("Deleted node {node}."));
                Ok(Step::Done)
            }
        }
    }
}

#[derive(Default)]
struct BactionM {
    phase: BactionPhase,
    param_node: i32,
    kind: String,
    f1: Option<Vec2>,
    f2: Option<Vec2>,
    sel: SelectPhase,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum BactionPhase {
    #[default]
    ChooseParam,
    ChooseType,
    FrameFirst,
    FrameSecond,
    SelectObjects,
}

impl Interactive for BactionM {
    fn name(&self) -> &'static str {
        "BACTION"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        if dynamic_block(s.doc()?, MODEL_GRAPH).is_none_or(|m| m.parameters.is_empty()) {
            return Err(EngineError::Other("Add a parameter with BPARAMETER first.".into()));
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.phase {
            BactionPhase::ChooseParam => Prompt::new("Enter parameter name or node ID", Accept::TEXT),
            BactionPhase::ChooseType => Prompt::new("Enter action type [Move/Stretch/Scale/Rotate/Flip/Lookup]", KW)
                .kw(&["Move", "Stretch", "Scale", "Rotate", "Flip", "Lookup"])
                .default("Stretch"),
            BactionPhase::FrameFirst => Prompt::new("Specify first corner of stretch frame", Accept::POINT),
            BactionPhase::FrameSecond => {
                let p = Prompt::new("Specify opposite corner", Accept::POINT);
                if let Some(a) = self.f1 { p.base(a) } else { p }
            }
            BactionPhase::SelectObjects => self.sel.prompt(),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.phase {
            BactionPhase::ChooseParam => {
                let (Input::Text(t) | Input::Keyword(t)) = i else { return Ok(Step::Continue) };
                let m = dynamic_block(s.doc()?, MODEL_GRAPH).ok_or_else(|| EngineError::Other("No dynamic parameters.".into()))?;
                self.param_node = resolve_param_node(&m, &json!(t.trim()), "baction")?;
                self.phase = BactionPhase::ChooseType;
                Ok(Step::Continue)
            }
            BactionPhase::ChooseType => {
                let kw = match i {
                    Input::Enter => "stretch".to_string(),
                    Input::Keyword(k) | Input::Text(k) => k.trim().to_ascii_lowercase(),
                    _ => return Ok(Step::Continue),
                };
                self.kind = match kw.as_str() {
                    "m" | "move" => "move",
                    "s" | "stretch" => "stretch",
                    "sc" | "scale" => "scale",
                    "r" | "rotate" => "rotate",
                    "f" | "flip" => "flip",
                    "l" | "lookup" => "lookup",
                    _ => return Err(EngineError::Other(format!("Unknown action type \"{kw}\"."))),
                }
                .to_string();
                if self.kind == "lookup" {
                    let res = run_baction(s, &json!({ "type": "lookup", "parameter": self.param_node }))?;
                    s.echo(format!("Added lookup action node {}.", res["node"]));
                    return Ok(Step::Done);
                }
                if self.kind == "stretch" {
                    self.phase = BactionPhase::FrameFirst;
                    return Ok(Step::Continue);
                }
                self.finish_or_select(s)
            }
            BactionPhase::FrameFirst => {
                let pt = match i {
                    Input::Point(p) => p,
                    Input::Deferred(d) => d.at,
                    _ => return Ok(Step::Continue),
                };
                self.f1 = Some(pt);
                self.phase = BactionPhase::FrameSecond;
                Ok(Step::Continue)
            }
            BactionPhase::FrameSecond => {
                let pt = match i {
                    Input::Point(p) => p,
                    Input::Deferred(d) => d.at,
                    _ => return Ok(Step::Continue),
                };
                self.f2 = Some(pt);
                self.finish_or_select(s)
            }
            BactionPhase::SelectObjects => match self.sel.feed(s, &i)? {
                SelOutcome::More => Ok(Step::Continue),
                SelOutcome::Empty => Ok(Step::Done),
                SelOutcome::Done(hs) => {
                    let handles: Vec<String> = hs.into_iter().map(|h| h.hex()).collect();
                    self.commit_action(s, &handles)
                }
            },
        }
    }
}

impl BactionM {
    fn finish_or_select(&mut self, s: &mut Session) -> Result<Step> {
        if !s.selection().is_empty() {
            let handles: Vec<String> = s.selection().iter().map(|h| h.hex()).collect();
            return self.commit_action(s, &handles);
        }
        self.sel = SelectPhase::default();
        self.phase = BactionPhase::SelectObjects;
        Ok(Step::Continue)
    }

    fn commit_action(&mut self, s: &mut Session, handles: &[String]) -> Result<Step> {
        let mut p = json!({
            "type": self.kind,
            "parameter": self.param_node,
            "handles": handles,
        });
        if let (Some(a), Some(b), Some(o)) = (self.f1, self.f2, p.as_object_mut()) {
            o.insert("frame".into(), json!([[a.x, a.y], [b.x, b.y]]));
        }
        let res = run_baction(s, &p)?;
        s.echo(format!("Added {} action node {}.", self.kind, res["node"]));
        Ok(Step::Done)
    }
}

#[derive(Default)]
struct BvstateM {
    op: Option<String>,
}

impl Interactive for BvstateM {
    fn name(&self) -> &'static str {
        "BVSTATE"
    }
    fn begin(&mut self, _s: &mut Session) -> Result<Step> {
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.op.as_deref() {
            None => Prompt::new("Enter visibility option [New/Current/Delete/Show/Hide/?]", KW)
                .kw(&["New", "Current", "Delete", "Show", "Hide", "?"])
                .default("?"),
            Some("new") => Prompt::new("Enter new visibility state name", Accept::TEXT),
            Some("current") => Prompt::new("Enter visibility state to make default", Accept::TEXT),
            Some("delete") => Prompt::new("Enter visibility state name to delete", Accept::TEXT),
            _ => Prompt::new("Enter visibility state name", Accept::TEXT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Some(op) = self.op.clone() {
            let (Input::Text(name) | Input::Keyword(name)) = i else { return Ok(Step::Done) };
            run_bvstate(s, &json!({ "op": op, "name": name.trim(), "state": name.trim() }))?;
            s.echo(format!("Visibility state \"{}\" updated ({op}).", name.trim()));
            return Ok(Step::Done);
        }
        let kw = match i {
            Input::Enter => "?".to_string(),
            Input::Keyword(k) | Input::Text(k) => k.trim().to_ascii_lowercase(),
            _ => return Ok(Step::Continue),
        };
        match kw.as_str() {
            "?" | "list" => {
                let res = run_bvstate(s, &json!({ "op": "list" }))?;
                let names: Vec<&str> = res["states"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect()).unwrap_or_default();
                s.echo(if names.is_empty() {
                    "No visibility states defined.".to_string()
                } else {
                    format!("Visibility states: {}", names.join(", "))
                });
                Ok(Step::Done)
            }
            "n" | "new" => {
                self.op = Some("new".into());
                Ok(Step::Continue)
            }
            "c" | "current" => {
                self.op = Some("current".into());
                Ok(Step::Continue)
            }
            "d" | "delete" => {
                self.op = Some("delete".into());
                Ok(Step::Continue)
            }
            "s" | "show" => {
                self.op = Some("show".into());
                Ok(Step::Continue)
            }
            "h" | "hide" => {
                self.op = Some("hide".into());
                Ok(Step::Continue)
            }
            _ => Err(EngineError::Other(format!("Unknown option \"{kw}\"."))),
        }
    }
}
