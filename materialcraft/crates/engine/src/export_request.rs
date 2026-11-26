use std::collections::HashMap;

use serde_json::Value as J;

use materialcraft_core::Graph;
use materialcraft_formats::Depth;

use crate::commands::{bad, strp, u64p};
use crate::{ColorSpace, ExportSize, Result, Session};

const CMD: &str = "file.export";

/// What `file.export` cooks and writes
#[derive(Clone, Debug, PartialEq)]
pub struct ExportRequest {
    pub graph: usize,
    /// File extension, one of `EXPORT_FORMATS`
    pub format: String,
    /// File names with $(graph), $(identifier) and $(usage)
    pub pattern: String,
    /// `None` keeps each output's own format
    pub depth: Option<Depth>,
    /// `None` keeps the graph's size
    pub size: Option<ExportSize>,
    /// Output identifiers to write; `None` writes every connected output
    pub outputs: Option<Vec<String>>,
    /// Color space per output identifier; outputs without one follow their usage
    pub color_spaces: HashMap<String, ColorSpace>,
}

impl ExportRequest {
    /// Every connected output of graph `graph` as PNG at the graph's size
    pub fn new(graph: usize) -> Self {
        Self { graph, format: "png".into(), pattern: "$(graph)_$(identifier)".into(), depth: None, size: None, outputs: None, color_spaces: HashMap::new() }
    }

    /// Read `file.export` params, checking them against the session's package
    pub fn from_params(s: &Session, p: &J) -> Result<Self> {
        let graph_index = u64p(p, "graph").map_or(s.graph, |index| index as usize);
        let graph = s.pkg.graph(graph_index)?;

        let format = strp(p, "format").unwrap_or("png").trim_start_matches('.').to_ascii_lowercase();
        if !materialcraft_formats::EXPORT_FORMATS.iter().any(|(extension, _)| *extension == format) {
            return Err(bad(CMD, format!("unknown format `{format}`")));
        }

        let depth = match u64p(p, "depth") {
            None => None,
            Some(8) => Some(Depth::U8),
            Some(16) => Some(Depth::U16),
            Some(32) => Some(Depth::F32),
            Some(depth) => return Err(bad(CMD, format!("depth must be 8, 16 or 32, not {depth}"))),
        };

        let identifiers = output_identifiers(graph);
        let outputs = match p.get("outputs") {
            None | Some(J::Null) => None,
            Some(J::Array(items)) => Some(selected_outputs(items, &identifiers, &graph.name)?),
            Some(_) => return Err(bad(CMD, "`outputs` must be a list of output identifiers")),
        };

        Ok(Self {
            graph: graph_index,
            format,
            pattern: strp(p, "pattern").unwrap_or("$(graph)_$(identifier)").to_string(),
            depth,
            size: export_size(p.get("size"))?,
            outputs,
            color_spaces: color_spaces(p.get("colorSpaces"), &identifiers, &graph.name)?,
        })
    }

    /// Whether the output named `identifier` is written
    pub fn includes(&self, identifier: &str) -> bool {
        self.outputs.as_ref().is_none_or(|outputs| outputs.iter().any(|output| output == identifier))
    }

    /// The color space output `identifier` with `usage` is written in
    pub fn color_space(&self, identifier: &str, usage: &str) -> ColorSpace {
        self.color_spaces.get(identifier).copied().unwrap_or_else(|| ColorSpace::for_usage(usage))
    }
}

fn output_identifiers(graph: &Graph) -> Vec<String> {
    graph.nodes.iter().filter(|node| node.def == "output").map(materialcraft_eval::identifier).collect()
}

fn selected_outputs(items: &[J], identifiers: &[String], graph: &str) -> Result<Vec<String>> {
    if items.is_empty() {
        return Err(bad(CMD, "no outputs selected"));
    }

    items
        .iter()
        .map(|item| {
            let identifier = item.as_str().ok_or_else(|| bad(CMD, "`outputs` must be a list of output identifiers"))?;
            known_output(identifier, identifiers, graph).map(str::to_string)
        })
        .collect()
}

fn color_spaces(value: Option<&J>, identifiers: &[String], graph: &str) -> Result<HashMap<String, ColorSpace>> {
    let entries = match value {
        None | Some(J::Null) => return Ok(HashMap::new()),
        Some(J::Object(entries)) => entries,
        Some(_) => return Err(bad(CMD, "`colorSpaces` must map output identifiers to srgb, linear or raw")),
    };

    entries
        .iter()
        .map(|(identifier, space)| {
            let space =
                space.as_str().and_then(ColorSpace::from_id).ok_or_else(|| bad(CMD, format!("`{identifier}`: the color space must be srgb, linear or raw")))?;
            Ok((known_output(identifier, identifiers, graph)?.to_string(), space))
        })
        .collect()
}

fn known_output<'a>(identifier: &'a str, identifiers: &[String], graph: &str) -> Result<&'a str> {
    if identifiers.iter().any(|known| known == identifier) { Ok(identifier) } else { Err(bad(CMD, format!("graph `{graph}` has no output `{identifier}`"))) }
}

/// `size`: the larger side in pixels (0 = the graph's size) or `[width, height]`
fn export_size(value: Option<&J>) -> Result<Option<ExportSize>> {
    let pixels = |value: &J| value.as_u64().or_else(|| value.as_f64().filter(|f| f.is_finite() && *f >= 0.0).map(|f| f.round() as u64));
    let clamp = |pixels: u64| pixels.min(u64::from(u32::MAX)) as u32;

    match value {
        None | Some(J::Null) => Ok(None),
        Some(J::Array(sides)) => match sides.as_slice() {
            [width, height] => match (pixels(width), pixels(height)) {
                (Some(width), Some(height)) => Ok(Some(ExportSize::Exact([clamp(width), clamp(height)]))),
                _ => Err(bad(CMD, "`size` [width, height] takes two pixel counts")),
            },
            _ => Err(bad(CMD, "`size` [width, height] takes two pixel counts")),
        },
        Some(value) => match pixels(value) {
            Some(0) => Ok(None),
            Some(side) => Ok(Some(ExportSize::LargerSide(clamp(side)))),
            None => Err(bad(CMD, "`size` is a pixel count or [width, height]")),
        },
    }
}
