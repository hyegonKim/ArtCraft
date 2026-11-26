//! Plot styles: PLOTSTYLE / -PLOTSTYLE (an object's or the current named plot style, plus
//! command-line `.ctb`/`.stb` import and export) and STYLESMANAGER / -STYLESMANAGER (the plot style
//! tables: list, view, create, edit, delete, load and save in AutoCAD binary/zlib `.ctb`/`.stb`,
//! uncompressed PIA, or CADCraft JSON format).
//!
//! Tables the drawing keeps live in `Drawing::plot_style_tables` and are saved with it; they
//! shadow built-in tables of the same name. Editing a built-in table stores an edited copy in
//! the drawing.

use cadcraft_doc::{Drawing, PlotStyle, PlotStyleKind, PlotStyleTable};
use serde_json::{Value, json};

use super::super::file::{base64_decode, base64_encode};
use super::super::targets;
use super::{CommandSpec, Result, Session, bad, str_param};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("plotstyle", "Plot Style...", run_plotstyle)
            .menu(&["Format", "Plot Style..."])
            .params(
                "{action?: import|export, name?: \"ByLayer\"|\"ByBlock\"|style name|table (import/export), handles?|handle? (default: the selection; nothing selected sets CPLOTSTYLE), path?, format?: binary|pia|json, text?, data?} (no params: reports current style and named styles)",
            ),
        CommandSpec::new("-plotstyle", "Plot Style", run_dash_plotstyle)
            .params("as PLOTSTYLE (typed: option prompts [?/Current/Import/Export] or plot style name)")
            .interactive(|_| Ok(Box::new(DashPlotStyleM::default()))),
        CommandSpec::new("stylesmanager", "Plot Style Manager...", run_stylesmanager)
            .menu(&["File", "Plot Styles..."])
            .params(
                "{action?: list (default)|get|new|set|delete|load|save, name?: table (\"x.ctb\"/\"x.stb\"), kind?: colorDependent|named (new), from?: table to copy (new), description?, style?: {name, color?: \"#rrggbb\"|null, grayscale?, screening?: 0-100, linetype?: name|null, lineweight?: mm|null, end?, join?, fill?, description?} (set), removeStyle?: name (set, named tables), path? (load/save; .ctb/.stb defaults to AutoCAD binary PIA), format?: binary|ctb|stb|pia|json (save), text? (load), data? (base64, load)}",
            ),
        CommandSpec::new("-stylesmanager", "Plot Style Manager", run_stylesmanager)
            .params("as STYLESMANAGER (typed: option prompts [List/Import/Export/Delete])")
            .interactive(|_| Ok(Box::new(StylesManagerM::default()))),
    ]
}

// ---------- PLOTSTYLE / -PLOTSTYLE ----------

/// The style names an object can be given: those of the named tables the layouts use, else of
/// every named table the drawing can use.
fn named_styles(d: &Drawing) -> Vec<String> {
    let mut used: Vec<PlotStyleTable> =
        d.layouts.iter().filter_map(|l| cadcraft_doc::plot_style_table(d, &l.page.plot_style_table)).filter(|t| !t.is_color_dependent()).collect();
    if used.is_empty() {
        used = cadcraft_doc::plot_style_table_names(d)
            .iter()
            .filter_map(|n| cadcraft_doc::plot_style_table(d, n))
            .filter(|t| !t.is_color_dependent())
            .collect();
    }
    let mut out: Vec<String> = Vec::new();
    for s in used.iter().flat_map(|t| t.styles.iter()) {
        if !out.iter().any(|n| n.eq_ignore_ascii_case(&s.name)) {
            out.push(s.name.clone());
        }
    }
    out
}

/// A plot style property value: `ByLayer`, `ByBlock` or a style name (at most 255 characters).
fn style_value(cmd: &str, v: &str) -> Result<String> {
    let v = v.trim();
    if v.is_empty() {
        return Err(bad(cmd, "the plot style name is empty"));
    }
    Ok(if v.eq_ignore_ascii_case("bylayer") {
        "ByLayer".into()
    } else if v.eq_ignore_ascii_case("byblock") {
        "ByBlock".into()
    } else {
        v.chars().take(255).collect()
    })
}

fn run_plotstyle_import_export(cmd: &str, s: &mut Session, p: &Value, action: &str) -> Result<Value> {
    match action {
        "import" | "load" => load_table(cmd, s, p),
        "export" | "save" => save_table(cmd, s, p, true),
        other => Err(bad(cmd, format!("unknown plotstyle action `{other}` (import or export)"))),
    }
}

fn run_plotstyle(s: &mut Session, p: &Value) -> Result<Value> {
    run_plotstyle_cmd("plotstyle", s, p)
}

fn run_dash_plotstyle(s: &mut Session, p: &Value) -> Result<Value> {
    run_plotstyle_cmd("-plotstyle", s, p)
}

fn run_plotstyle_cmd(cmd: &str, s: &mut Session, p: &Value) -> Result<Value> {
    if let Some(act) = str_param(p, "action").map(str::to_ascii_lowercase) {
        return run_plotstyle_import_export(cmd, s, p, &act);
    }
    let Some(name) = p.get("name") else {
        let d = s.doc()?;
        return Ok(json!({ "current": d.header.str("CPLOTSTYLE", "ByLayer"), "styles": named_styles(d) }));
    };
    let name = style_value(cmd, name.as_str().ok_or_else(|| bad(cmd, "`name` must be a plot style name"))?)?;
    let handles = targets(s, p)?;
    if handles.is_empty() {
        s.doc_mut()?.header.set_str("CPLOTSTYLE", &name);
        return Ok(json!({ "current": name }));
    }
    let mut changed = 0usize;
    for h in &handles {
        if super::super::curves::is_locked(s, *h) {
            continue;
        }
        let d = s.doc_mut()?;
        if d.entity(*h).is_none() {
            return Err(bad(cmd, format!("no object {}", h.hex())));
        }
        d.modify_entity(*h, |e| e.common.plot_style = name.clone())?;
        changed += 1;
    }
    Ok(json!({ "changed": changed, "plotStyle": name }))
}

#[derive(Clone, Debug, Default)]
enum DashStage {
    #[default]
    Root,
    CurrentName,
    ImportPath,
    ExportTable,
    ExportPath(String),
}

#[derive(Default)]
struct DashPlotStyleM {
    stage: DashStage,
}

const TEXT_ACCEPT: Accept = Accept { point: false, number: false, text: true, select: false, enter: true };
const KW_ACCEPT: Accept = Accept { point: false, number: false, text: false, select: false, enter: true };

impl Interactive for DashPlotStyleM {
    fn name(&self) -> &'static str {
        "-plotstyle"
    }

    fn prompt(&self, s: &Session) -> Prompt {
        let current = s.doc().map(|d| d.header.str("CPLOTSTYLE", "ByLayer").to_string()).unwrap_or_else(|_| "ByLayer".into());
        match &self.stage {
            DashStage::Root => Prompt::new(format!("Current plot style is \"{current}\". Enter plot style"), TEXT_ACCEPT)
                .kw(&["?", "Current", "Import", "Export"])
                .default(current),
            DashStage::CurrentName => Prompt::new("Enter plot style name (ByLayer, ByBlock, or named style)", TEXT_ACCEPT).default(current),
            DashStage::ImportPath => Prompt::new("Enter .ctb or .stb file path to import", TEXT_ACCEPT),
            DashStage::ExportTable => Prompt::new("Enter plot style table name to export", TEXT_ACCEPT).default("monochrome.ctb"),
            DashStage::ExportPath(tbl) => Prompt::new(format!("Enter output .ctb/.stb file path for {tbl}"), TEXT_ACCEPT),
        }
    }

    fn input(&mut self, s: &mut Session, input: Input) -> Result<Step> {
        let typed = match &input {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()).filter(|t| !t.is_empty()),
            _ => None,
        };
        match &self.stage {
            DashStage::Root => match (input, typed) {
                (Input::Enter, _) => Ok(Step::Done),
                (_, Some(k)) if k.eq_ignore_ascii_case("?") => {
                    if let Ok(d) = s.doc() {
                        s.echo(format!("Named plot styles: {}", named_styles(d).join(", ")));
                    }
                    Ok(Step::Continue)
                }
                (_, Some(k)) if k.eq_ignore_ascii_case("Current") => {
                    self.stage = DashStage::CurrentName;
                    Ok(Step::Continue)
                }
                (_, Some(k)) if k.eq_ignore_ascii_case("Import") => {
                    self.stage = DashStage::ImportPath;
                    Ok(Step::Continue)
                }
                (_, Some(k)) if k.eq_ignore_ascii_case("Export") => {
                    self.stage = DashStage::ExportTable;
                    Ok(Step::Continue)
                }
                (_, Some(style)) => {
                    run_plotstyle_cmd("-plotstyle", s, &json!({ "name": style })).map_err(clean_err)?;
                    Ok(Step::Done)
                }
                _ => Ok(Step::Done),
            },
            DashStage::CurrentName => {
                if let Some(style) = typed {
                    run_plotstyle_cmd("-plotstyle", s, &json!({ "name": style })).map_err(clean_err)?;
                }
                Ok(Step::Done)
            }
            DashStage::ImportPath => {
                let path = typed.ok_or_else(|| EngineError::Other("Requires a .ctb or .stb file path.".into()))?;
                let r = load_table("-plotstyle", s, &json!({ "path": path })).map_err(clean_err)?;
                if let Some(n) = r.get("name").and_then(Value::as_str) {
                    s.echo(format!("Imported plot style table {n}"));
                }
                Ok(Step::Done)
            }
            DashStage::ExportTable => {
                let tbl = typed.unwrap_or_else(|| "monochrome.ctb".into());
                self.stage = DashStage::ExportPath(tbl);
                Ok(Step::Continue)
            }
            DashStage::ExportPath(tbl) => {
                let tbl = tbl.clone();
                let path = typed.ok_or_else(|| EngineError::Other("Requires an output file path.".into()))?;
                let r = save_table("-plotstyle", s, &json!({ "name": tbl, "path": path }), true).map_err(clean_err)?;
                if let Some(out_path) = r.get("path").and_then(Value::as_str) {
                    s.echo(format!("Saved {tbl} to {out_path}"));
                }
                Ok(Step::Done)
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
enum MgrStage {
    #[default]
    Root,
    ImportPath,
    ExportTable,
    ExportPath(String),
    DeleteTable,
}

#[derive(Default)]
struct StylesManagerM {
    stage: MgrStage,
}

impl Interactive for StylesManagerM {
    fn name(&self) -> &'static str {
        "-stylesmanager"
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        match &self.stage {
            MgrStage::Root => Prompt::new("Plot Style Manager", KW_ACCEPT).kw(&["List", "Import", "Export", "Delete"]).default("List"),
            MgrStage::ImportPath => Prompt::new("Enter .ctb or .stb file path to import", TEXT_ACCEPT),
            MgrStage::ExportTable => Prompt::new("Enter plot style table name to export", TEXT_ACCEPT).default("monochrome.ctb"),
            MgrStage::ExportPath(tbl) => Prompt::new(format!("Enter output .ctb/.stb file path for {tbl}"), TEXT_ACCEPT),
            MgrStage::DeleteTable => Prompt::new("Enter plot style table name to delete", TEXT_ACCEPT),
        }
    }

    fn input(&mut self, s: &mut Session, input: Input) -> Result<Step> {
        let typed = match &input {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()).filter(|t| !t.is_empty()),
            _ => None,
        };
        match &self.stage {
            MgrStage::Root => match typed.as_deref() {
                None | Some("List") => {
                    if let Ok(d) = s.doc() {
                        s.echo(format!("Plot style tables: {}", cadcraft_doc::plot_style_table_names(d).join(", ")));
                    }
                    Ok(Step::Done)
                }
                Some("Import") => {
                    self.stage = MgrStage::ImportPath;
                    Ok(Step::Continue)
                }
                Some("Export") => {
                    self.stage = MgrStage::ExportTable;
                    Ok(Step::Continue)
                }
                Some("Delete") => {
                    self.stage = MgrStage::DeleteTable;
                    Ok(Step::Continue)
                }
                _ => Ok(Step::Done),
            },
            MgrStage::ImportPath => {
                let path = typed.ok_or_else(|| EngineError::Other("Requires a .ctb or .stb file path.".into()))?;
                let r = load_table("-stylesmanager", s, &json!({ "path": path })).map_err(clean_err)?;
                if let Some(n) = r.get("name").and_then(Value::as_str) {
                    s.echo(format!("Loaded plot style table {n}"));
                }
                Ok(Step::Done)
            }
            MgrStage::ExportTable => {
                let tbl = typed.unwrap_or_else(|| "monochrome.ctb".into());
                self.stage = MgrStage::ExportPath(tbl);
                Ok(Step::Continue)
            }
            MgrStage::ExportPath(tbl) => {
                let tbl = tbl.clone();
                let path = typed.ok_or_else(|| EngineError::Other("Requires an output file path.".into()))?;
                save_table("-stylesmanager", s, &json!({ "name": tbl, "path": path }), true).map_err(clean_err)?;
                Ok(Step::Done)
            }
            MgrStage::DeleteTable => {
                let tbl = typed.ok_or_else(|| EngineError::Other("Requires a table name.".into()))?;
                run_stylesmanager(s, &json!({ "action": "delete", "name": tbl })).map_err(clean_err)?;
                Ok(Step::Done)
            }
        }
    }
}

fn clean_err(e: EngineError) -> EngineError {
    match e {
        EngineError::BadParams { msg, .. } => EngineError::Other(msg),
        other => other,
    }
}

// ---------- STYLESMANAGER ----------

fn table_summary(d: &Drawing, t: &PlotStyleTable) -> Value {
    let in_drawing = d.plot_style_tables.iter().any(|x| x.name.eq_ignore_ascii_case(&t.name));
    json!({
        "name": t.name,
        "description": t.description,
        "kind": if t.is_color_dependent() { "colorDependent" } else { "named" },
        "styles": t.styles.len(),
        "inDrawing": in_drawing,
        "builtin": PlotStyleTable::builtin(&t.name).is_some() && !in_drawing,
    })
}

fn table_name(cmd: &str, p: &Value) -> Result<String> {
    let n = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(cmd, "`name` (the table) is required"))?;
    Ok(n.chars().take(255).collect())
}

/// The table `name` as the drawing sees it.
fn find_table(cmd: &str, d: &Drawing, name: &str) -> Result<PlotStyleTable> {
    cadcraft_doc::plot_style_table(d, name).ok_or_else(|| bad(cmd, format!("no plot style table `{name}`")))
}

/// Store `t` in the drawing, replacing a table of the same name.
fn store(d: &mut Drawing, t: PlotStyleTable) {
    match d.plot_style_tables.iter_mut().find(|x| x.name.eq_ignore_ascii_case(&t.name)) {
        Some(x) => *x = t,
        None => d.plot_style_tables.push(t),
    }
}

fn opt_str_field(cmd: &str, v: &Value, k: &str) -> Result<Option<Option<String>>> {
    match v.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) if s.trim().is_empty() || s.eq_ignore_ascii_case("object") => Ok(Some(None)),
        Some(Value::String(s)) => Ok(Some(Some(s.trim().chars().take(255).collect()))),
        Some(_) => Err(bad(cmd, format!("`{k}` must be a string or null"))),
    }
}

/// Apply the fields present in `v` to style `st`.
fn edit_style(cmd: &str, st: &mut PlotStyle, v: &Value) -> Result<()> {
    if let Some(c) = opt_str_field(cmd, v, "color")? {
        st.color = match c {
            None => None,
            Some(h) => {
                Some(cadcraft_color::Rgb::parse_hex(h.trim_start_matches('#')).ok_or_else(|| bad(cmd, "`color` must be \"#rrggbb\" or null"))?)
            }
        };
    }
    if let Some(lt) = opt_str_field(cmd, v, "linetype")? {
        st.linetype = lt;
    }
    if let Some(d) = opt_str_field(cmd, v, "description")? {
        st.description = d.unwrap_or_default();
    }
    if let Some(g) = v.get("grayscale") {
        st.grayscale = g.as_bool().ok_or_else(|| bad(cmd, "`grayscale` must be true or false"))?;
    }
    if let Some(sc) = v.get("screening") {
        let n = sc.as_f64().filter(|n| n.is_finite() && (0.0..=100.0).contains(n)).ok_or_else(|| bad(cmd, "`screening` must be 0 to 100"))?;
        st.screening = n.round() as u8;
    }
    match v.get("lineweight") {
        None => {}
        Some(Value::Null) => st.lineweight = None,
        Some(Value::String(s)) if s.eq_ignore_ascii_case("object") => st.lineweight = None,
        Some(w) => {
            let mm = w
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=2.11).contains(n))
                .ok_or_else(|| bad(cmd, "`lineweight` must be 0 to 2.11 mm or null"))?;
            st.lineweight = Some((mm * 100.0).round() as u16);
        }
    }
    edit_style_enums(cmd, st, v)
}

fn edit_style_enums(cmd: &str, st: &mut PlotStyle, v: &Value) -> Result<()> {
    fn field<T: serde::de::DeserializeOwned>(cmd: &str, v: &Value, k: &str, msg: &str) -> Result<Option<T>> {
        v.get(k).map(|x| serde_json::from_value(x.clone()).map_err(|_| bad(cmd, msg))).transpose()
    }
    if let Some(e) = field(cmd, v, "end", "`end` must be object, butt, square, round or diamond")? {
        st.end = e;
    }
    if let Some(j) = field(cmd, v, "join", "`join` must be object, miter, bevel, round or diamond")? {
        st.join = j;
    }
    if let Some(f) = field(
        cmd,
        v,
        "fill",
        "`fill` must be object, solid, checkerboard, crosshatch, diamonds, horizontalBars, slantLeft, slantRight, squareDots or verticalBars",
    )? {
        st.fill = f;
    }
    Ok(())
}

fn load_table(cmd: &str, s: &mut Session, p: &Value) -> Result<Value> {
    let raw_data = str_param(p, "data").or_else(|| str_param(p, "base64"));
    let path_param = str_param(p, "path");
    let bytes = match (raw_data, str_param(p, "text"), path_param) {
        (Some(b64), _, _) => base64_decode(b64).ok_or_else(|| bad(cmd, "invalid base64 `data`"))?,
        (None, Some(t), _) => t.as_bytes().to_vec(),
        (None, None, Some(_path)) => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                crate::capped_read::read_capped(std::path::Path::new(_path), cadcraft_doc::MAX_TABLE_FILE)
                    .map_err(|e| bad(cmd, format!("{_path}: {e}")))?
            }
            #[cfg(target_arch = "wasm32")]
            return Err(bad(cmd, "files can't be read here: pass `data` or `text`"));
        }
        (None, None, None) => return Err(bad(cmd, "`path`, `data`, or `text` is required")),
    };
    let hint = str_param(p, "name").or(path_param);
    let mut t = PlotStyleTable::from_file_bytes(&bytes, hint).map_err(|e| bad(cmd, e))?;
    if let Some(n) = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()) {
        t.name = n.chars().take(255).collect();
    }
    if t.name.is_empty() {
        return Err(bad(cmd, "the table has no name: pass `name`"));
    }
    t = ensure_matching_extension(cmd, t)?;
    let out = table_summary(s.doc()?, &t);
    store(s.doc_mut()?, t);
    Ok(out)
}

fn ensure_matching_extension(cmd: &str, mut t: PlotStyleTable) -> Result<PlotStyleTable> {
    let ext = if t.is_color_dependent() { ".ctb" } else { ".stb" };
    let lower = t.name.to_ascii_lowercase();
    if !lower.ends_with(ext) {
        if lower.ends_with(".ctb") || lower.ends_with(".stb") {
            return Err(bad(cmd, format!("a {} table's name ends in {ext}", if ext == ".ctb" { "colour-dependent" } else { "named" })));
        }
        t.name.push_str(ext);
    }
    Ok(t)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveFormat {
    BinaryPia,
    TextPia,
    Json,
}

fn resolve_save_format(cmd: &str, p: &Value, path: Option<&str>, prefer_binary_without_path: bool) -> Result<SaveFormat> {
    match str_param(p, "format").map(str::to_ascii_lowercase).as_deref() {
        Some("binary" | "ctb" | "stb" | "zlib") => Ok(SaveFormat::BinaryPia),
        Some("pia" | "text" | "uncompress") => Ok(SaveFormat::TextPia),
        Some("json") => Ok(SaveFormat::Json),
        Some(other) => Err(bad(cmd, format!("unknown `format` `{other}` (binary, pia, or json)"))),
        None => match path {
            Some(pt) => {
                let lower = pt.to_ascii_lowercase();
                if lower.ends_with(".ctb") || lower.ends_with(".stb") {
                    Ok(SaveFormat::BinaryPia)
                } else {
                    Ok(SaveFormat::Json)
                }
            }
            None if prefer_binary_without_path => Ok(SaveFormat::BinaryPia),
            None => Ok(SaveFormat::Json),
        },
    }
}

fn save_table(cmd: &str, s: &mut Session, p: &Value, prefer_binary_without_path: bool) -> Result<Value> {
    let name = table_name(cmd, p)?;
    let table = find_table(cmd, s.doc()?, &name)?;
    let path_opt = str_param(p, "path");
    let fmt = resolve_save_format(cmd, p, path_opt, prefer_binary_without_path)?;
    match (fmt, path_opt) {
        (SaveFormat::Json, None) => Ok(json!({ "name": name, "text": table.to_file_text() })),
        (SaveFormat::TextPia, None) => Ok(json!({ "name": name, "format": "pia", "text": table.to_pia_text() })),
        (SaveFormat::BinaryPia, None) => {
            let bytes = table.to_binary_bytes();
            Ok(json!({ "name": name, "format": "binary", "data": base64_encode(&bytes), "bytes": bytes.len() }))
        }
        (_, Some(_path)) => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let (bytes, fmt_tag) = match fmt {
                    SaveFormat::BinaryPia => {
                        let lower = _path.to_ascii_lowercase();
                        let bin = if lower.ends_with(".ctb") {
                            cadcraft_doc::write_binary_ctb(&table).map_err(|e| bad(cmd, e))?
                        } else if lower.ends_with(".stb") {
                            cadcraft_doc::write_binary_stb(&table).map_err(|e| bad(cmd, e))?
                        } else {
                            table.to_binary_bytes()
                        };
                        (bin, "binary")
                    }
                    SaveFormat::TextPia => (table.to_pia_text().into_bytes(), "pia"),
                    SaveFormat::Json => (table.to_file_text().into_bytes(), "json"),
                };
                crate::atomic_write::write_atomic(std::path::Path::new(_path), &bytes).map_err(|e| bad(cmd, format!("{_path}: {e}")))?;
                Ok(json!({ "name": name, "path": _path, "format": fmt_tag, "bytes": bytes.len() }))
            }
            #[cfg(target_arch = "wasm32")]
            Err(bad(cmd, "files can't be written here: leave out `path` to get `text` or `data`"))
        }
    }
}

fn new_table(cmd: &str, s: &mut Session, p: &Value) -> Result<Value> {
    let name = table_name(cmd, p)?;
    let d = s.doc()?;
    let mut t = match str_param(p, "from") {
        Some(from) => find_table(cmd, d, from)?,
        None => {
            let named = match str_param(p, "kind").map(str::to_ascii_lowercase).as_deref() {
                None if name.to_ascii_lowercase().ends_with(".stb") => true,
                None | Some("colordependent" | "ctb") => false,
                Some("named" | "stb") => true,
                Some(_) => return Err(bad(cmd, "`kind` must be colorDependent or named")),
            };
            if named { PlotStyleTable::named("", "", Vec::new()) } else { PlotStyleTable::color_dependent("", "", |_| None) }
        }
    };
    t.name = name;
    t = ensure_matching_extension(cmd, t)?;
    if d.plot_style_tables.iter().any(|x| x.name.eq_ignore_ascii_case(&t.name)) {
        return Err(bad(cmd, format!("plot style table `{}` already exists", t.name)));
    }
    if let Some(desc) = str_param(p, "description") {
        t.description = desc.chars().take(255).collect();
    }
    let t = t.sanitized();
    let out = table_summary(d, &t);
    store(s.doc_mut()?, t);
    Ok(out)
}

fn set_table(cmd: &str, s: &mut Session, p: &Value) -> Result<Value> {
    let name = table_name(cmd, p)?;
    let mut t = find_table(cmd, s.doc()?, &name)?;
    if let Some(desc) = str_param(p, "description") {
        t.description = desc.chars().take(255).collect();
    }
    if let Some(rm) = str_param(p, "removeStyle") {
        if t.is_color_dependent() {
            return Err(bad(cmd, "a colour-dependent table always has 255 styles"));
        }
        if rm.trim().eq_ignore_ascii_case(cadcraft_doc::NORMAL_STYLE) {
            return Err(bad(cmd, "the Normal style can't be removed"));
        }
        let before = t.styles.len();
        t.styles.retain(|x| !x.name.eq_ignore_ascii_case(rm.trim()));
        if t.styles.len() == before {
            return Err(bad(cmd, format!("`{name}` has no style `{rm}`")));
        }
    }
    if let Some(v) = p.get("style") {
        let sname = v.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
        let sname = sname.ok_or_else(|| bad(cmd, "`style.name` is required"))?;
        let idx = match t.styles.iter().position(|x| x.name.eq_ignore_ascii_case(sname)) {
            Some(i) => i,
            None if t.kind == PlotStyleKind::Named => {
                if t.styles.len() >= cadcraft_doc::MAX_NAMED_STYLES {
                    return Err(bad(cmd, "the table has too many styles"));
                }
                t.styles.push(PlotStyle::object(&sname.chars().take(255).collect::<String>()));
                t.styles.len() - 1
            }
            None => return Err(bad(cmd, format!("`{name}` has no style `{sname}` (colour-dependent styles are Color_1 to Color_255)"))),
        };
        if let Some(st) = t.styles.get_mut(idx) {
            edit_style(cmd, st, v)?;
        }
    }
    let t = t.sanitized();
    let out = serde_json::to_value(&t).unwrap_or(Value::Null);
    store(s.doc_mut()?, t);
    Ok(out)
}

fn run_stylesmanager(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "stylesmanager";
    let action = str_param(p, "action").unwrap_or("list").to_ascii_lowercase();
    match action.as_str() {
        "list" => {
            let d = s.doc()?;
            let tables: Vec<Value> = cadcraft_doc::plot_style_table_names(d)
                .iter()
                .filter_map(|n| cadcraft_doc::plot_style_table(d, n))
                .map(|t| table_summary(d, &t))
                .collect();
            Ok(json!({ "tables": tables }))
        }
        "get" => {
            let d = s.doc()?;
            let t = find_table(cmd, d, &table_name(cmd, p)?)?;
            let mut v = serde_json::to_value(&t).unwrap_or(Value::Null);
            if let (Some(o), Value::Object(sum)) = (v.as_object_mut(), table_summary(d, &t)) {
                o.insert("inDrawing".into(), sum.get("inDrawing").cloned().unwrap_or(Value::Null));
                o.insert("builtin".into(), sum.get("builtin").cloned().unwrap_or(Value::Null));
            }
            Ok(v)
        }
        "new" => new_table(cmd, s, p),
        "set" => set_table(cmd, s, p),
        "delete" => {
            let name = table_name(cmd, p)?;
            let d = s.doc_mut()?;
            let before = d.plot_style_tables.len();
            d.plot_style_tables.retain(|t| !t.name.eq_ignore_ascii_case(&name));
            if d.plot_style_tables.len() == before {
                return Err(bad(cmd, format!("the drawing keeps no plot style table `{name}` (built-in tables can't be deleted)")));
            }
            Ok(json!({ "deleted": name }))
        }
        "load" | "import" => load_table(cmd, s, p),
        "save" | "export" => save_table(cmd, s, p, false),
        other => Err(bad(cmd, format!("unknown action `{other}` (list, get, new, set, delete, load, save)"))),
    }
}
