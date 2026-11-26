//! Board background grid settings (`board.grid`), stored in [`crate::Prefs`].

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use crate::cmd::CommandSpec;
use crate::params::get;
use crate::{Result, Session, bad, spec};

/// Allowed spacing range on the page.
pub const SPACING: (f64, f64) = (4.0, 512.0);
/// Allowed cells per strong line on graph paper.
pub const MAJOR: (u32, u32) = (2, 12);
/// Allowed mark size multiplier range.
pub const SIZE: (f64, f64) = (0.5, 4.0);
/// Allowed mark contrast multiplier range.
pub const CONTRAST: (f64, f64) = (0.25, 4.0);

/// Style of a board's background grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GridStyle {
    #[default]
    Dots,
    Lines,
    Graph,
    None,
}

impl GridStyle {
    pub const ALL: [GridStyle; 4] = [GridStyle::Dots, GridStyle::Lines, GridStyle::Graph, GridStyle::None];

    pub fn id(self) -> &'static str {
        match self {
            GridStyle::Dots => "dots",
            GridStyle::Lines => "lines",
            GridStyle::Graph => "graph",
            GridStyle::None => "none",
        }
    }

    pub fn from_id(s: &str) -> Option<GridStyle> {
        GridStyle::ALL.into_iter().find(|g| g.id().eq_ignore_ascii_case(s))
    }
}

/// Board background grid preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardGrid {
    pub style: GridStyle,
    pub spacing: f64,
    pub major: u32,
    pub size: f64,
    pub contrast: f64,
}

impl Default for BoardGrid {
    fn default() -> Self {
        Self {
            style: GridStyle::Dots,
            spacing: 24.0,
            major: 5,
            size: 1.0,
            contrast: 1.0,
        }
    }
}

impl BoardGrid {
    /// Clamp every numeric field into its valid range, replacing non-finite numbers with defaults.
    pub fn sanitized(self) -> Self {
        let def = Self::default();
        let clamp_f = |v: f64, (lo, hi): (f64, f64), d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Self {
            style: self.style,
            spacing: clamp_f(self.spacing, SPACING, def.spacing),
            major: self.major.clamp(MAJOR.0, MAJOR.1),
            size: clamp_f(self.size, SIZE, def.size),
            contrast: clamp_f(self.contrast, CONTRAST, def.contrast),
        }
    }

    fn to_json(&self) -> Value {
        let g = self.clone().sanitized();
        json!({
            "style": g.style.id(),
            "spacing": g.spacing,
            "major": g.major,
            "size": g.size,
            "contrast": g.contrast,
        })
    }
}

/// Tolerant deserializer for `Prefs`: falls back to `BoardGrid::default()` if edited to something
/// unreadable, while clamping out-of-range fields.
pub(crate) fn read<'de, D>(deserializer: D) -> std::result::Result<BoardGrid, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value::<BoardGrid>(raw).map(BoardGrid::sanitized).unwrap_or_default())
}

fn num_field(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

fn grid_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "board.grid";
    let reset = match get(p, "reset") {
        None | Some(Value::Null) => false,
        Some(v) => v.as_bool().ok_or_else(|| bad(cmd, "`reset` must be true or false"))?,
    };
    let style = match get(p, "style") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let sv = v.as_str().ok_or_else(|| bad(cmd, "`style` must be dots, lines, graph or none"))?;
            Some(GridStyle::from_id(sv).ok_or_else(|| bad(cmd, format!("unknown grid style `{sv}` (dots|lines|graph|none)")))?)
        }
    };
    let spacing = num_field(cmd, p, "spacing")?;
    let size = num_field(cmd, p, "size")?;
    let contrast = num_field(cmd, p, "contrast")?;
    let major = match get(p, "major") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let n = v
                .as_i64()
                .or_else(|| v.as_f64().filter(|x| x.is_finite()).map(|x| x.round() as i64))
                .ok_or_else(|| bad(cmd, "`major` must be an integer"))?;
            Some(u32::try_from(n.clamp(i64::from(MAJOR.0), i64::from(MAJOR.1))).unwrap_or(MAJOR.0))
        }
    };

    let mut next = if reset { BoardGrid::default() } else { s.prefs.board_grid.clone().sanitized() };
    if let Some(st) = style {
        next.style = st;
    }
    if let Some(sp) = spacing {
        next.spacing = sp;
    }
    if let Some(mj) = major {
        next.major = mj;
    }
    if let Some(sz) = size {
        next.size = sz;
    }
    if let Some(ct) = contrast {
        next.contrast = ct;
    }
    next = next.sanitized();
    s.prefs.board_grid = next.clone();
    Ok(next.to_json())
}

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        "board.grid",
        "Board grid",
        "{style?: dots|lines|graph|none, spacing?, major?, size?, contrast?, reset?} → {style, spacing, major, size, contrast}  the background grid shown on boards",
        false,
        grid_cmd
    )]
}
