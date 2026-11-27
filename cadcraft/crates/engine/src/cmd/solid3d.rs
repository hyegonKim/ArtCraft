//! 3D Clay SDF / CSG Solid Modeling (`solid3d.*`), STL & OBJ Mesh Export (`solid3d.exportStl`,
//! `solid3d.exportObj`, `STLOUT`), and 3D-to-2D CAD View Projection (`FLATSHOT`, `VIEWBASE`,
//! `solid3d.projectTo2d`).

use cadcraft_color::Color;
use cadcraft_doc::{Common, Drawing, EntityKind, Handle, Layer, Line, LwPolyline, Solid3dEntry, Space};
use cadcraft_geom::sdf3d::{
    CsgOp, MAX_COORD_MAGNITUDE, OrthoView3d, Projected2d, SdfPrimitive, Solid3dNode, Transform3d, extract_isosurface, project_solid_to_2d, write_obj,
    write_stl_ascii, write_stl_binary,
};
use cadcraft_geom::{Bounds2, PolyVertex, Vec2, Vec3};
use serde_json::{Value, json};

use super::file::base64_encode;
use super::{CommandSpec, bad, bool_or, point_param, str_param};
use crate::{Result, Session};

const MAX_SOLIDS: usize = 512;
const MAX_DIMENSION: f64 = 1e5;
const DEFAULT_MESH_RES: usize = 36;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("solid3d.primitive", "3D Solid Primitive", run_primitive)
            .alias(&["box3d", "sphere3d", "cylinder3d", "cone3d", "torus3d", "capsule3d"])
            .params("{kind: \"box\"|\"sphere\"|\"cylinder\"|\"cone\"|\"torus\"|\"capsule\", size?: [w,d,h] | halfExtents?: [hx,hy,hz], radius?, baseRadius?, topRadius?, majorRadius?, minorRadius?, height?, a?: [x,y,z], b?: [x,y,z], position?: [x,y,z], rotationDeg?: [rx,ry,rz], scale?: [sx,sy,sz]|s, name?, layer?} → {id, name, bounds}"),
        CommandSpec::new("solid3d.boolean", "3D Clay Boolean", run_boolean)
            .params("{op: \"union\"|\"subtract\"|\"intersect\", leftId?: u64, rightId?: u64, smoothK?: f64 (0 = sharp CSG, >0 = smooth clay fillet), keepInputs?: bool, name?} → {id, name, op, smoothK, bounds}"),
        CommandSpec::new("solid3d.transform", "Transform 3D Solid", run_transform)
            .params("{id: u64, position?: [x,y,z], rotationDeg?: [rx,ry,rz], scale?: [sx,sy,sz]|s} → {id, bounds}"),
        CommandSpec::new("solid3d.list", "List 3D Solids", run_list)
            .params("{} → {solids: [{id, name, layer, bounds}]}")
            .noundo(),
        CommandSpec::new("solid3d.delete", "Delete 3D Solid", run_delete)
            .params("{id: u64} → {deleted}"),
        CommandSpec::new("solid3d.clear", "Clear 3D Solids", run_clear)
            .params("{} → {cleared}"),
        CommandSpec::new("solid3d.exportstl", "Export 3D STL", run_export_stl)
            .alias(&["stlout"])
            .params("{id?: u64 (default: union of all 3D solids), resolution?: 6..96 (default 36), binary?: bool (default true), path?} → {format: \"stl\", binary, vertices, triangles, bytes, path? | data, text?}")
            .noundo(),
        CommandSpec::new("solid3d.exportobj", "Export 3D OBJ", run_export_obj)
            .alias(&["objout"])
            .params("{id?: u64 (default: union of all 3D solids), resolution?: 6..96 (default 36), path?} → {format: \"obj\", vertices, triangles, bytes, path? | data, text}")
            .noundo(),
        CommandSpec::new("flatshot", "Flatshot 2D Projection", run_flatshot)
            .menu(&["Draw", "Flatshot 2D View"])
            .params("{id?: u64 (default: all 3D solids), view?: \"top\"|\"front\"|\"right\"|\"isometric\" (default \"top\"), at?: [x,y], scale?: f64 (default 1), resolution?: 6..96, creaseAngle?: deg (default 32), showHidden?: bool, layer?: \"SOLID2D\", hiddenLayer?: \"SOLID2D-HIDDEN\"} → {view, handles, polylines, lines, hiddenLines, bounds}"),
        CommandSpec::new("solid3d.projectto2d", "Flatshot 2D Projection", run_flatshot)
            .params("{id?: u64 (default: all 3D solids), view?: \"top\"|\"front\"|\"right\"|\"isometric\" (default \"top\"), at?: [x,y], scale?: f64 (default 1), resolution?: 6..96, creaseAngle?: deg (default 32), showHidden?: bool, layer?: \"SOLID2D\", hiddenLayer?: \"SOLID2D-HIDDEN\"} → {view, handles, polylines, lines, hiddenLines, bounds}"),
        CommandSpec::new("viewbase", "Base & Projected 3D Views", run_viewbase)
            .menu(&["Draw", "Base & Projected Views"])
            .params("{id?: u64 (default: all 3D solids), at?: [x,y], scale?: f64 (default 1), spacing?: f64, resolution?: 6..96, showHidden?: bool, layer?: \"SOLID2D\"} → {views, handles, count}"),
    ]
}

fn pos_num(cmd: &str, p: &Value, key: &str, default: Option<f64>) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => default.ok_or_else(|| bad(cmd, format!("`{key}` (positive number) is required"))),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && *x > 0.0 && *x <= MAX_DIMENSION)
            .ok_or_else(|| bad(cmd, format!("`{key}` must be in (0, {MAX_DIMENSION}]"))),
    }
}

fn nonneg_num(cmd: &str, p: &Value, key: &str, default: f64) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && *x >= 0.0 && *x <= MAX_DIMENSION)
            .ok_or_else(|| bad(cmd, format!("`{key}` must be in [0, {MAX_DIMENSION}]"))),
    }
}

fn vec3_value(v: &Value) -> Option<Vec3> {
    if let Some(arr) = v.as_array() {
        let x = arr.first()?.as_f64()?;
        let y = arr.get(1)?.as_f64()?;
        let z = arr.get(2).and_then(Value::as_f64).unwrap_or(0.0);
        let out = Vec3::new(x, y, z);
        return (out.is_finite() && out.x.abs() <= MAX_COORD_MAGNITUDE && out.y.abs() <= MAX_COORD_MAGNITUDE && out.z.abs() <= MAX_COORD_MAGNITUDE)
            .then_some(out);
    }
    let x = v.get("x")?.as_f64()?;
    let y = v.get("y")?.as_f64()?;
    let z = v.get("z").and_then(Value::as_f64).unwrap_or(0.0);
    let out = Vec3::new(x, y, z);
    (out.is_finite() && out.x.abs() <= MAX_COORD_MAGNITUDE && out.y.abs() <= MAX_COORD_MAGNITUDE && out.z.abs() <= MAX_COORD_MAGNITUDE).then_some(out)
}

fn vec3_opt(cmd: &str, p: &Value, key: &str, default: Vec3) -> Result<Vec3> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => vec3_value(v).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite [x, y, z] within +/-{MAX_COORD_MAGNITUDE}"))),
    }
}

fn parse_transform(cmd: &str, p: &Value) -> Result<Transform3d> {
    let translation = match p.get("position").or_else(|| p.get("translation")) {
        None | Some(Value::Null) => Vec3::ZERO,
        Some(v) => vec3_value(v).ok_or_else(|| bad(cmd, "`position` must be [x, y, z]"))?,
    };
    let rotation = if let Some(v) = p.get("rotationDeg") {
        let deg = vec3_value(v).ok_or_else(|| bad(cmd, "`rotationDeg` must be [rx, ry, rz]"))?;
        Vec3::new(deg.x.to_radians(), deg.y.to_radians(), deg.z.to_radians())
    } else if let Some(v) = p.get("rotation") {
        vec3_value(v).ok_or_else(|| bad(cmd, "`rotation` must be [rx, ry, rz] radians"))?
    } else {
        Vec3::ZERO
    };
    let scale = match p.get("scale") {
        None | Some(Value::Null) => Vec3::new(1.0, 1.0, 1.0),
        Some(v) if v.is_number() => {
            let s = v.as_f64().filter(|x| x.is_finite() && *x > 1e-4 && *x <= 1e4).ok_or_else(|| bad(cmd, "`scale` must be in (1e-4, 1e4]"))?;
            Vec3::new(s, s, s)
        }
        Some(v) => {
            let sv = vec3_value(v).ok_or_else(|| bad(cmd, "`scale` must be a number or [sx, sy, sz]"))?;
            if sv.x <= 1e-4 || sv.y <= 1e-4 || sv.z <= 1e-4 || sv.x > 1e4 || sv.y > 1e4 || sv.z > 1e4 {
                return Err(bad(cmd, "`scale` components must be in (1e-4, 1e4]"));
            }
            sv
        }
    };
    Ok(Transform3d { translation, rotation, scale })
}

fn parse_primitive(cmd: &str, p: &Value) -> Result<(&'static str, SdfPrimitive)> {
    let kind_raw =
        str_param(p, "kind").ok_or_else(|| bad(cmd, "`kind` (\"box\"|\"sphere\"|\"cylinder\"|\"cone\"|\"torus\"|\"capsule\") is required"))?;
    match kind_raw.trim().to_ascii_lowercase().as_str() {
        "box" | "box3d" => parse_box_prim(cmd, p),
        "sphere" | "sphere3d" => Ok(("Sphere", SdfPrimitive::Sphere3d { radius: pos_num(cmd, p, "radius", Some(10.0))? })),
        "cylinder" | "cylinder3d" => Ok((
            "Cylinder",
            SdfPrimitive::Cylinder3d { radius: pos_num(cmd, p, "radius", Some(8.0))?, height: pos_num(cmd, p, "height", Some(20.0))? },
        )),
        "cone" | "cone3d" => {
            let base_radius = match p.get("baseRadius") {
                Some(_) => pos_num(cmd, p, "baseRadius", None)?,
                None => pos_num(cmd, p, "radius", Some(10.0))?,
            };
            let top_radius = nonneg_num(cmd, p, "topRadius", 0.0)?;
            let height = pos_num(cmd, p, "height", Some(20.0))?;
            Ok(("Cone", SdfPrimitive::Cone3d { base_radius, top_radius, height }))
        }
        "torus" | "torus3d" => {
            let major_radius = pos_num(cmd, p, "majorRadius", Some(12.0))?;
            let minor_radius = pos_num(cmd, p, "minorRadius", Some(4.0))?;
            Ok(("Torus", SdfPrimitive::Torus3d { major_radius, minor_radius }))
        }
        "capsule" | "capsule3d" => parse_capsule_prim(cmd, p),
        other => Err(bad(cmd, format!("unknown 3D primitive kind `{other}`"))),
    }
}

fn parse_box_prim(cmd: &str, p: &Value) -> Result<(&'static str, SdfPrimitive)> {
    let half_extents = if let Some(v) = p.get("halfExtents") {
        let he = vec3_value(v).ok_or_else(|| bad(cmd, "`halfExtents` must be [hx, hy, hz]"))?;
        if he.x <= 0.0 || he.y <= 0.0 || he.z <= 0.0 || he.x > MAX_DIMENSION || he.y > MAX_DIMENSION || he.z > MAX_DIMENSION {
            return Err(bad(cmd, "`halfExtents` components must be positive"));
        }
        he
    } else if let Some(v) = p.get("size") {
        let sz = vec3_value(v).ok_or_else(|| bad(cmd, "`size` must be [w, d, h]"))?;
        if sz.x <= 0.0 || sz.y <= 0.0 || sz.z <= 0.0 || sz.x > MAX_DIMENSION || sz.y > MAX_DIMENSION || sz.z > MAX_DIMENSION {
            return Err(bad(cmd, "`size` components must be positive"));
        }
        sz * 0.5
    } else {
        let w = pos_num(cmd, p, "width", Some(20.0))?;
        let d = pos_num(cmd, p, "depth", Some(20.0))?;
        let h = pos_num(cmd, p, "height", Some(20.0))?;
        Vec3::new(w * 0.5, d * 0.5, h * 0.5)
    };
    Ok(("Box", SdfPrimitive::Box3d { half_extents }))
}

fn parse_capsule_prim(cmd: &str, p: &Value) -> Result<(&'static str, SdfPrimitive)> {
    let radius = pos_num(cmd, p, "radius", Some(5.0))?;
    let hh = pos_num(cmd, p, "height", Some(20.0))? * 0.5;
    let a = vec3_opt(cmd, p, "a", Vec3::new(0.0, 0.0, -hh))?;
    let b = vec3_opt(cmd, p, "b", Vec3::new(0.0, 0.0, hh))?;
    Ok(("Capsule", SdfPrimitive::Capsule3d { a, b, radius }))
}

fn bounds3_json(node: &Solid3dNode) -> Value {
    let b = node.bounds();
    if !b.is_finite() {
        return Value::Null;
    }
    json!({
        "min": [b.min.x, b.min.y, b.min.z],
        "max": [b.max.x, b.max.y, b.max.z],
    })
}

fn run_primitive(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid3d.primitive";
    let (base_name, prim) = parse_primitive(cmd, p)?;
    let transform = parse_transform(cmd, p)?;
    let d = s.doc_mut()?;
    if d.solids3d.len() >= MAX_SOLIDS {
        return Err(bad(cmd, format!("at most {MAX_SOLIDS} 3D solids can be kept in one drawing")));
    }
    let id = d.alloc_solid3d_id();
    let name = str_param(p, "name")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| n.chars().take(64).collect::<String>())
        .unwrap_or_else(|| format!("{base_name}_{id}"));
    let layer = str_param(p, "layer")
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().take(64).collect::<String>())
        .unwrap_or_else(|| d.header.str("CLAYER", "0"));
    d.ensure_layer(&layer);
    let node = Solid3dNode::primitive_at(prim, transform);
    let bounds = bounds3_json(&node);
    d.solids3d.push(Solid3dEntry { id, name: name.clone(), layer: layer.clone(), node });
    Ok(json!({ "id": id, "name": name, "layer": layer, "bounds": bounds }))
}

fn run_boolean(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid3d.boolean";
    let op_str = str_param(p, "op").ok_or_else(|| bad(cmd, "`op` (\"union\"|\"subtract\"|\"intersect\") is required"))?;
    let op = CsgOp::parse(op_str).ok_or_else(|| bad(cmd, format!("unknown boolean operation `{op_str}`")))?;
    let smooth_k = nonneg_num(cmd, p, "smoothK", 0.0)?;
    let keep_inputs = bool_or(p, "keepInputs", false);

    let d = s.doc_mut()?;
    if d.solids3d.len() < 2 {
        return Err(bad(cmd, "at least two 3D solids are needed for a boolean operation"));
    }
    let default_pair = || {
        let len = d.solids3d.len();
        (d.solids3d[len - 2].id, d.solids3d[len - 1].id)
    };
    let (def_l, def_r) = default_pair();
    let left_id = match p.get("leftId") {
        None | Some(Value::Null) => def_l,
        Some(v) => v.as_u64().ok_or_else(|| bad(cmd, "`leftId` must be a solid id"))?,
    };
    let right_id = match p.get("rightId") {
        None | Some(Value::Null) => def_r,
        Some(v) => v.as_u64().ok_or_else(|| bad(cmd, "`rightId` must be a solid id"))?,
    };
    if left_id == right_id {
        return Err(bad(cmd, "`leftId` and `rightId` must refer to distinct 3D solids"));
    }
    let left = d.solids3d.iter().find(|e| e.id == left_id).cloned().ok_or_else(|| bad(cmd, format!("no 3D solid with id {left_id}")))?;
    let right = d.solids3d.iter().find(|e| e.id == right_id).cloned().ok_or_else(|| bad(cmd, format!("no 3D solid with id {right_id}")))?;
    if keep_inputs && d.solids3d.len() >= MAX_SOLIDS {
        return Err(bad(cmd, format!("at most {MAX_SOLIDS} 3D solids can be kept in one drawing")));
    }

    if !keep_inputs {
        d.solids3d.retain(|e| e.id != left_id && e.id != right_id);
    }
    let id = d.alloc_solid3d_id();
    let name = str_param(p, "name")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| n.chars().take(64).collect::<String>())
        .unwrap_or_else(|| format!("{}_{id}", op.as_str()));
    let node = Solid3dNode::boolean(op, smooth_k, left.node, right.node);
    let bounds = bounds3_json(&node);
    d.solids3d.push(Solid3dEntry { id, name: name.clone(), layer: left.layer, node });
    Ok(json!({ "id": id, "name": name, "op": op.as_str(), "smoothK": smooth_k, "bounds": bounds }))
}

fn run_transform(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid3d.transform";
    let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "`id` (u64) is required"))?;
    let tf = parse_transform(cmd, p)?;
    let d = s.doc_mut()?;
    let entry = d.solids3d.iter_mut().find(|e| e.id == id).ok_or_else(|| bad(cmd, format!("no 3D solid with id {id}")))?;
    entry.node = entry.node.clone().transformed(tf);
    let bounds = bounds3_json(&entry.node);
    Ok(json!({ "id": id, "bounds": bounds }))
}

fn run_list(s: &mut Session, _p: &Value) -> Result<Value> {
    let d = s.doc()?;
    let items: Vec<Value> =
        d.solids3d.iter().map(|e| json!({ "id": e.id, "name": e.name, "layer": e.layer, "bounds": bounds3_json(&e.node) })).collect();
    Ok(json!({ "solids": items, "count": items.len() }))
}

fn run_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid3d.delete";
    let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "`id` (u64) is required"))?;
    let d = s.doc_mut()?;
    let before = d.solids3d.len();
    d.solids3d.retain(|e| e.id != id);
    if d.solids3d.len() == before {
        return Err(bad(cmd, format!("no 3D solid with id {id}")));
    }
    Ok(json!({ "deleted": id }))
}

fn run_clear(s: &mut Session, _p: &Value) -> Result<Value> {
    let d = s.doc_mut()?;
    let cleared = d.solids3d.len();
    d.solids3d.clear();
    Ok(json!({ "cleared": cleared }))
}

fn target_solid_node(cmd: &str, d: &Drawing, p: &Value) -> Result<(String, Solid3dNode)> {
    if d.solids3d.is_empty() {
        return Err(bad(cmd, "no 3D solids in this drawing; create one with `solid3d.primitive` first"));
    }
    if let Some(v) = p.get("id")
        && !v.is_null()
    {
        let id = v.as_u64().ok_or_else(|| bad(cmd, "`id` must be a solid id"))?;
        let found = d.solids3d.iter().find(|e| e.id == id).ok_or_else(|| bad(cmd, format!("no 3D solid with id {id}")))?;
        return Ok((found.name.clone(), found.node.clone()));
    }
    let mut iter = d.solids3d.iter();
    let Some(first) = iter.next() else {
        return Err(bad(cmd, "no 3D solids in this drawing"));
    };
    let name = if d.solids3d.len() == 1 { first.name.clone() } else { "assembly".to_string() };
    let combined = iter.fold(first.node.clone(), |acc, item| Solid3dNode::boolean(CsgOp::Union, 0.0, acc, item.node.clone()));
    Ok((name, combined))
}

fn parse_resolution(cmd: &str, p: &Value) -> Result<usize> {
    match p.get("resolution") {
        None | Some(Value::Null) => Ok(DEFAULT_MESH_RES),
        Some(v) => {
            let r = v.as_u64().ok_or_else(|| bad(cmd, "`resolution` must be an integer between 6 and 96"))?;
            if !(6..=96).contains(&r) {
                return Err(bad(cmd, "`resolution` must be between 6 and 96"));
            }
            Ok(r as usize)
        }
    }
}

fn write_export_bytes(cmd: &str, path: &str, bytes: &[u8]) -> Result<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.len() > 1024 || trimmed.chars().any(char::is_control) {
        return Err(bad(cmd, "invalid export `path`"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::atomic_write::write_atomic_making_folders(std::path::Path::new(trimmed), bytes).map_err(|e| bad(cmd, format!("{trimmed}: {e}")))
    }
    #[cfg(target_arch = "wasm32")]
    {
        super::exchange::deliver_file(cmd, trimmed, bytes, super::file::deliver())
    }
}

fn run_export_stl(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid3d.exportstl";
    let res = parse_resolution(cmd, p)?;
    let binary = bool_or(p, "binary", true);
    let (name, node) = target_solid_node(cmd, s.doc()?, p)?;
    let mesh = extract_isosurface(&node, res);
    if mesh.triangles.is_empty() {
        return Err(bad(cmd, "the 3D solid has an empty isosurface at this resolution"));
    }
    let (bytes, ascii_text) = if binary {
        (write_stl_binary(&name, &mesh), None)
    } else {
        let txt = write_stl_ascii(&name, &mesh);
        (txt.as_bytes().to_vec(), Some(txt))
    };
    if let Some(path) = str_param(p, "path") {
        write_export_bytes(cmd, path, &bytes)?;
        return Ok(json!({
            "format": "stl",
            "binary": binary,
            "path": path,
            "vertices": mesh.vertices.len(),
            "triangles": mesh.triangles.len(),
            "bytes": bytes.len(),
        }));
    }
    let mut out = json!({
        "format": "stl",
        "binary": binary,
        "vertices": mesh.vertices.len(),
        "triangles": mesh.triangles.len(),
        "bytes": bytes.len(),
        "data": base64_encode(&bytes),
    });
    if let (Some(txt), Some(obj)) = (ascii_text, out.as_object_mut()) {
        obj.insert("text".into(), json!(txt));
    }
    Ok(out)
}

fn run_export_obj(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "solid3d.exportobj";
    let res = parse_resolution(cmd, p)?;
    let (name, node) = target_solid_node(cmd, s.doc()?, p)?;
    let mesh = extract_isosurface(&node, res);
    if mesh.triangles.is_empty() {
        return Err(bad(cmd, "the 3D solid has an empty isosurface at this resolution"));
    }
    let text = write_obj(&name, &mesh);
    let bytes = text.as_bytes();
    if let Some(path) = str_param(p, "path") {
        write_export_bytes(cmd, path, bytes)?;
        return Ok(json!({
            "format": "obj",
            "path": path,
            "vertices": mesh.vertices.len(),
            "triangles": mesh.triangles.len(),
            "bytes": bytes.len(),
        }));
    }
    Ok(json!({
        "format": "obj",
        "vertices": mesh.vertices.len(),
        "triangles": mesh.triangles.len(),
        "bytes": bytes.len(),
        "data": base64_encode(bytes),
        "text": text,
    }))
}

struct Placement2d<'a> {
    space: &'a Space,
    origin: Vec2,
    scale: f64,
    layer: &'a str,
    hidden_layer: &'a str,
}

struct PlacedCounts {
    handles: Vec<Handle>,
    polylines: usize,
    lines: usize,
    hidden_lines: usize,
    bounds: Bounds2,
}

fn place_projected_entities(d: &mut Drawing, proj: &Projected2d, place: &Placement2d<'_>) -> Result<PlacedCounts> {
    d.ensure_layer(place.layer);
    if !proj.hidden_segments.is_empty() && d.layer(place.hidden_layer).is_none() {
        let mut hlayer = Layer::new(place.hidden_layer);
        hlayer.color = Color::Index(8);
        d.layers.push(hlayer);
    }
    let mut counts = PlacedCounts { handles: Vec::new(), polylines: 0, lines: 0, hidden_lines: 0, bounds: Bounds2::EMPTY };
    let map_pt = |pt: Vec2| place.origin + pt * place.scale;

    for chain in &proj.visible_polylines {
        let mapped: Vec<Vec2> = chain.iter().map(|&pt| map_pt(pt)).collect();
        for &pt in &mapped {
            counts.bounds.add(pt);
        }
        let common = Common { layer: place.layer.to_string(), ..Common::default() };
        if mapped.len() == 2 {
            let kind = EntityKind::Line(Line { a: mapped[0].to3(0.0), b: mapped[1].to3(0.0) });
            if let Ok(h) = d.add(place.space, common, kind) {
                counts.handles.push(h);
                counts.lines += 1;
            }
        } else if mapped.len() >= 3 {
            let closed = mapped.first().zip(mapped.last()).is_some_and(|(a, b)| a.near(*b, 1e-4 * place.scale.max(1.0)));
            let slice = if closed { &mapped[..mapped.len() - 1] } else { &mapped[..] };
            if slice.len() >= 2 {
                let vertices = slice.iter().map(|&pt| PolyVertex::new(pt)).collect();
                let kind = EntityKind::LwPolyline(LwPolyline { vertices, closed, const_width: 0.0, elevation: 0.0, plinegen: false });
                if let Ok(h) = d.add(place.space, common, kind) {
                    counts.handles.push(h);
                    counts.polylines += 1;
                }
            }
        }
    }

    for &[a, b] in &proj.hidden_segments {
        let (pa, pb) = (map_pt(a), map_pt(b));
        counts.bounds.add(pa);
        counts.bounds.add(pb);
        let common = Common { layer: place.hidden_layer.to_string(), ..Common::default() };
        let kind = EntityKind::Line(Line { a: pa.to3(0.0), b: pb.to3(0.0) });
        if let Ok(h) = d.add(place.space, common, kind) {
            counts.handles.push(h);
            counts.hidden_lines += 1;
        }
    }
    Ok(counts)
}

fn parse_proj_scale(cmd: &str, p: &Value) -> Result<f64> {
    match p.get("scale") {
        None | Some(Value::Null) => Ok(1.0),
        Some(v) => v.as_f64().filter(|k| k.is_finite() && *k >= 1e-4 && *k <= 1e4).ok_or_else(|| bad(cmd, "`scale` must be in [1e-4, 1e4]")),
    }
}

fn parse_layer_name<'a>(p: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    str_param(p, key).map(str::trim).filter(|s| !s.is_empty() && s.len() <= 64).unwrap_or(fallback)
}

fn run_flatshot(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "flatshot";
    let view = match str_param(p, "view") {
        None => OrthoView3d::Top,
        Some(v) => OrthoView3d::parse(v).ok_or_else(|| bad(cmd, format!("unknown view `{v}` (expected top, front, right, or isometric)")))?,
    };
    let origin = match p.get("at").or_else(|| p.get("origin")) {
        None | Some(Value::Null) => Vec2::ZERO,
        Some(_) => point_param(p, "at")
            .or_else(|| point_param(p, "origin"))
            .filter(|pt| pt.x.abs() <= MAX_COORD_MAGNITUDE && pt.y.abs() <= MAX_COORD_MAGNITUDE)
            .ok_or_else(|| bad(cmd, "`at` must be a finite [x, y] point"))?,
    };
    let scale = parse_proj_scale(cmd, p)?;
    let res = parse_resolution(cmd, p)?;
    let crease_deg = match p.get("creaseAngle") {
        None | Some(Value::Null) => 32.0,
        Some(v) => {
            v.as_f64().filter(|a| a.is_finite() && (5.0..=89.0).contains(a)).ok_or_else(|| bad(cmd, "`creaseAngle` must be in [5, 89] degrees"))?
        }
    };
    let show_hidden = bool_or(p, "showHidden", false);
    let layer = parse_layer_name(p, "layer", "SOLID2D").to_string();
    let hidden_layer = parse_layer_name(p, "hiddenLayer", "SOLID2D-HIDDEN").to_string();

    let (_, node) = target_solid_node(cmd, s.doc()?, p)?;
    let proj = project_solid_to_2d(&node, view, res, crease_deg.to_radians(), show_hidden);
    let space = s.space();
    let placed =
        place_projected_entities(s.doc_mut()?, &proj, &Placement2d { space: &space, origin, scale, layer: &layer, hidden_layer: &hidden_layer })?;
    s.set_selection(placed.handles.clone());
    let bounds_val = if placed.bounds.is_empty() {
        Value::Null
    } else {
        json!({
            "min": [placed.bounds.min.x, placed.bounds.min.y],
            "max": [placed.bounds.max.x, placed.bounds.max.y],
            "width": placed.bounds.width(),
            "height": placed.bounds.height(),
        })
    };
    Ok(json!({
        "view": view.as_str(),
        "handles": placed.handles.iter().map(|h| h.hex()).collect::<Vec<_>>(),
        "polylines": placed.polylines,
        "lines": placed.lines,
        "hiddenLines": placed.hidden_lines,
        "bounds": bounds_val,
    }))
}

fn run_viewbase(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "viewbase";
    let base_at = match p.get("at") {
        None | Some(Value::Null) => Vec2::ZERO,
        Some(_) => point_param(p, "at")
            .filter(|pt| pt.x.abs() <= MAX_COORD_MAGNITUDE && pt.y.abs() <= MAX_COORD_MAGNITUDE)
            .ok_or_else(|| bad(cmd, "`at` must be a finite [x, y] point"))?,
    };
    let scale = parse_proj_scale(cmd, p)?;
    let res = parse_resolution(cmd, p)?;
    let show_hidden = bool_or(p, "showHidden", false);
    let layer = parse_layer_name(p, "layer", "SOLID2D").to_string();
    let hidden_layer = parse_layer_name(p, "hiddenLayer", "SOLID2D-HIDDEN").to_string();

    let (_, node) = target_solid_node(cmd, s.doc()?, p)?;
    let b3 = node.bounds();
    let default_gap = (b3.max_extent() * scale * 1.6).max(25.0);
    let gap = match p.get("spacing") {
        None | Some(Value::Null) => default_gap,
        Some(v) => v.as_f64().filter(|g| g.is_finite() && *g > 0.0 && *g <= MAX_DIMENSION).ok_or_else(|| bad(cmd, "`spacing` must be positive"))?,
    };

    let layout = [
        (OrthoView3d::Front, base_at),
        (OrthoView3d::Top, base_at + Vec2::new(0.0, gap)),
        (OrthoView3d::Right, base_at + Vec2::new(gap, 0.0)),
        (OrthoView3d::Isometric, base_at + Vec2::new(gap, gap)),
    ];
    let space = s.space();
    let mut all_handles = Vec::new();
    let mut views_out = Vec::new();
    for (view, origin) in layout {
        let proj = project_solid_to_2d(&node, view, res, 32_f64.to_radians(), show_hidden);
        let placed =
            place_projected_entities(s.doc_mut()?, &proj, &Placement2d { space: &space, origin, scale, layer: &layer, hidden_layer: &hidden_layer })?;
        views_out.push(json!({
            "view": view.as_str(),
            "origin": [origin.x, origin.y],
            "entities": placed.handles.len(),
        }));
        all_handles.extend(placed.handles);
    }
    let count = all_handles.len();
    s.set_selection(all_handles.clone());
    Ok(json!({
        "views": views_out,
        "handles": all_handles.iter().map(|h| h.hex()).collect::<Vec<_>>(),
        "count": count,
    }))
}

#[cfg(test)]
mod tests {
    use cadcraft_doc::EntityKind;
    use serde_json::json;

    use crate::Session;

    #[test]
    fn clay_csg_part_exports_stl_obj_projects_flatshot_and_dimensions_in_2d() {
        let mut s = Session::new();
        let flange = s.execute("solid3d.primitive", &json!({ "kind": "box", "size": [100.0, 60.0, 20.0], "name": "Flange" })).unwrap();
        let boss = s
            .execute(
                "solid3d.primitive",
                &json!({ "kind": "cylinder", "radius": 18.0, "height": 30.0, "position": [0.0, 0.0, 15.0], "name": "Boss" }),
            )
            .unwrap();
        let unioned = s
            .execute(
                "solid3d.boolean",
                &json!({
                    "op": "union",
                    "leftId": flange["id"],
                    "rightId": boss["id"],
                    "smoothK": 4.0,
                    "name": "ClayBody"
                }),
            )
            .unwrap();
        let bore = s.execute("solid3d.primitive", &json!({ "kind": "cylinder", "radius": 8.0, "height": 80.0, "name": "Bore" })).unwrap();
        let final_part = s
            .execute(
                "solid3d.boolean",
                &json!({
                    "op": "subtract",
                    "leftId": unioned["id"],
                    "rightId": bore["id"],
                    "smoothK": 0.0,
                    "name": "FinishedBracket"
                }),
            )
            .unwrap();
        let pid = final_part["id"].as_u64().unwrap();

        // Export binary & ASCII STL + Wavefront OBJ (via camelCase command lookups too!)
        let stl_bin = s.execute("solid3d.exportStl", &json!({ "id": pid, "resolution": 28, "binary": true })).unwrap();
        assert_eq!(stl_bin["format"], "stl");
        assert!(stl_bin["triangles"].as_u64().unwrap() > 100);
        assert!(stl_bin["data"].as_str().is_some_and(|d| !d.is_empty()));

        let stl_ascii = s.execute("solid3d.exportStl", &json!({ "id": pid, "resolution": 24, "binary": false })).unwrap();
        assert!(stl_ascii["text"].as_str().is_some_and(|t| t.starts_with("solid FinishedBracket")));

        let obj = s.execute("solid3d.exportObj", &json!({ "id": pid, "resolution": 24 })).unwrap();
        assert!(obj["text"].as_str().is_some_and(|t| t.contains("\nf ")));

        // Project onto Top 2D CAD view via `solid3d.projectTo2d` alias of `flatshot`
        let shot =
            s.execute("solid3d.projectTo2d", &json!({ "id": pid, "view": "top", "at": [0.0, 0.0], "resolution": 28, "layer": "PART2D" })).unwrap();
        assert_eq!(shot["view"], "top");
        assert!(!shot["handles"].as_array().unwrap().is_empty());
        let width = shot["bounds"]["width"].as_f64().unwrap();
        let height = shot["bounds"]["height"].as_f64().unwrap();
        assert!((width - 100.0).abs() < 1.5, "projected width={width}");
        assert!((height - 60.0).abs() < 1.5, "projected height={height}");

        // Place a native 2D linear dimension directly across the projected bounds!
        let min_x = shot["bounds"]["min"][0].as_f64().unwrap();
        let max_x = shot["bounds"]["max"][0].as_f64().unwrap();
        let min_y = shot["bounds"]["min"][1].as_f64().unwrap();
        let dim = s
            .execute(
                "dimlinear",
                &json!({
                    "p1": [min_x, min_y],
                    "p2": [max_x, min_y],
                    "at": [0.0, min_y - 15.0]
                }),
            )
            .unwrap();
        assert!(dim["handle"].as_str().is_some());
        let has_dim = s.doc().unwrap().model.iter().any(|e| matches!(e.kind, EntityKind::Dimension(_)));
        assert!(has_dim, "2D linear dimension placed directly on projected 3D solid view");

        // Multi-view `viewbase` creates Front, Top, Right, and Isometric 2D CAD views
        let vb = s.execute("viewbase", &json!({ "id": pid, "at": [250.0, 0.0], "resolution": 20, "showHidden": true })).unwrap();
        assert_eq!(vb["views"].as_array().unwrap().len(), 4);
        assert!(vb["count"].as_u64().unwrap() >= 4);

        // Undo restores cleanly
        s.execute("undo", &json!({})).unwrap();
    }
}
