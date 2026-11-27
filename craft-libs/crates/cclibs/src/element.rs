use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{AseColor, AseColorType, Error};

/// All 19 Creative Cloud Library (`LIBS v4.20.2`) MIME element types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CcElementType {
    /// `application/vnd.adobe.element.color+dcx`
    Color,
    /// `application/vnd.adobe.element.colortheme+dcx`
    ColorTheme,
    /// `application/vnd.adobe.element.gradient+dcx`
    Gradient,
    /// `application/vnd.adobe.element.characterstyle+dcx`
    CharacterStyle,
    /// `application/vnd.adobe.element.paragraphstyle+dcx`
    ParagraphStyle,
    /// `application/vnd.adobe.element.brush+dcx`
    Brush,
    /// `application/vnd.adobe.element.layerstyle+dcx`
    LayerStyle,
    /// `application/vnd.adobe.element.image+dcx`
    Image,
    /// `application/vnd.adobe.element.3d+dcx`
    ThreeD,
    /// `application/vnd.adobe.element.material+dcx`
    Material,
    /// `application/vnd.adobe.element.light+dcx`
    Light,
    /// `application/vnd.adobe.element.audio+dcx`
    Audio,
    /// `application/vnd.adobe.element.video+dcx`
    Video,
    /// `application/vnd.adobe.element.animation+dcx`
    Animation,
    /// `application/vnd.adobe.element.template+dcx`
    Template,
    /// `application/vnd.adobe.element.component+dcx`
    Component,
    /// `application/vnd.adobe.element.text+dcx`
    Text,
    /// `application/vnd.adobe.element.agc+json`
    AgcJson,
    /// `application/vnd.adobe.element.agc+dcx`
    AgcDcx,
}

impl CcElementType {
    /// Complete array of all 19 `LIBS v4.20.2` element types.
    pub const ALL: [Self; 19] = [
        Self::Color,
        Self::ColorTheme,
        Self::Gradient,
        Self::CharacterStyle,
        Self::ParagraphStyle,
        Self::Brush,
        Self::LayerStyle,
        Self::Image,
        Self::ThreeD,
        Self::Material,
        Self::Light,
        Self::Audio,
        Self::Video,
        Self::Animation,
        Self::Template,
        Self::Component,
        Self::Text,
        Self::AgcJson,
        Self::AgcDcx,
    ];

    /// Canonical `application/vnd.adobe.element.*` MIME string for this element type.
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Color => "application/vnd.adobe.element.color+dcx",
            Self::ColorTheme => "application/vnd.adobe.element.colortheme+dcx",
            Self::Gradient => "application/vnd.adobe.element.gradient+dcx",
            Self::CharacterStyle => "application/vnd.adobe.element.characterstyle+dcx",
            Self::ParagraphStyle => "application/vnd.adobe.element.paragraphstyle+dcx",
            Self::Brush => "application/vnd.adobe.element.brush+dcx",
            Self::LayerStyle => "application/vnd.adobe.element.layerstyle+dcx",
            Self::Image => "application/vnd.adobe.element.image+dcx",
            Self::ThreeD => "application/vnd.adobe.element.3d+dcx",
            Self::Material => "application/vnd.adobe.element.material+dcx",
            Self::Light => "application/vnd.adobe.element.light+dcx",
            Self::Audio => "application/vnd.adobe.element.audio+dcx",
            Self::Video => "application/vnd.adobe.element.video+dcx",
            Self::Animation => "application/vnd.adobe.element.animation+dcx",
            Self::Template => "application/vnd.adobe.element.template+dcx",
            Self::Component => "application/vnd.adobe.element.component+dcx",
            Self::Text => "application/vnd.adobe.element.text+dcx",
            Self::AgcJson => "application/vnd.adobe.element.agc+json",
            Self::AgcDcx => "application/vnd.adobe.element.agc+dcx",
        }
    }

    /// Short element suffix after `application/vnd.adobe.element.` (for example `"color+dcx"`).
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Color => "color+dcx",
            Self::ColorTheme => "colortheme+dcx",
            Self::Gradient => "gradient+dcx",
            Self::CharacterStyle => "characterstyle+dcx",
            Self::ParagraphStyle => "paragraphstyle+dcx",
            Self::Brush => "brush+dcx",
            Self::LayerStyle => "layerstyle+dcx",
            Self::Image => "image+dcx",
            Self::ThreeD => "3d+dcx",
            Self::Material => "material+dcx",
            Self::Light => "light+dcx",
            Self::Audio => "audio+dcx",
            Self::Video => "video+dcx",
            Self::Animation => "animation+dcx",
            Self::Template => "template+dcx",
            Self::Component => "component+dcx",
            Self::Text => "text+dcx",
            Self::AgcJson => "agc+json",
            Self::AgcDcx => "agc+dcx",
        }
    }

    /// Resolve a [`CcElementType`] from its full MIME string (or `application/vnd.adobe.agc+*` alias).
    pub fn from_mime(mime: &str) -> Option<Self> {
        let trimmed = mime.trim();
        if let Some(suffix) = trimmed.strip_prefix("application/vnd.adobe.element.") {
            return Self::from_short_name(suffix);
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "application/vnd.adobe.agc+json" => Some(Self::AgcJson),
            "application/vnd.adobe.agc+dcx" => Some(Self::AgcDcx),
            _ => Self::ALL.into_iter().find(|t| t.mime_type().eq_ignore_ascii_case(trimmed)),
        }
    }

    /// Resolve a [`CcElementType`] from its short suffix (`"color+dcx"`, `"3d+dcx"`, `"agc+json"`, ...).
    pub fn from_short_name(short: &str) -> Option<Self> {
        let lower = short.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|t| t.short_name() == lower)
    }

    /// Whether this element type uses a `+dcx` composite representation layout.
    pub const fn is_dcx_container(self) -> bool {
        !matches!(self, Self::AgcJson)
    }
}

impl fmt::Display for CcElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mime_type())
    }
}

impl Serialize for CcElementType {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.mime_type())
    }
}

impl<'de> Deserialize<'de> for CcElementType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_mime(&s).or_else(|| Self::from_short_name(&s)).ok_or_else(|| serde::de::Error::custom(Error::UnsupportedElementMime(s)))
    }
}

/// Structured color payload used inside `color+dcx` (`application/vnd.adobe.color+json`) and
/// `colortheme+dcx` (`application/vnd.adobe.colortheme+json`) representations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CcColorValue {
    /// Color space mode (`"RGB"`, `"CMYK"`, `"Lab"`, `"Gray"`).
    pub mode: String,
    /// Numeric channel values in normalized `0.0..=1.0` (or CIE Lab `L`, `a`, `b`).
    pub components: Vec<f32>,
    /// Optional spot/process/global color kind (`"process"`, `"spot"`, `"global"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_type: Option<String>,
    /// Optional spot color book/pantone or swatch name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_color_name: Option<String>,
}

impl CcColorValue {
    /// Construct a [`CcColorValue`] from an [`AseColor`] and [`AseColorType`].
    pub fn from_ase(color: AseColor, color_type: AseColorType) -> Self {
        let (mode, components) = match color {
            AseColor::Rgb { r, g, b } => ("RGB".to_string(), vec![r, g, b]),
            AseColor::Cmyk { c, m, y, k } => ("CMYK".to_string(), vec![c, m, y, k]),
            AseColor::Lab { l, a, b } => ("Lab".to_string(), vec![l, a, b]),
            AseColor::Gray { value } => ("Gray".to_string(), vec![value]),
        };
        let kind = match color_type {
            AseColorType::Global => "global",
            AseColorType::Spot => "spot",
            AseColorType::Process => "process",
        };
        Self { mode, components, color_type: Some(kind.to_string()), spot_color_name: None }
    }

    /// Convert this [`CcColorValue`] into an `ASEF` [`AseColor`] and [`AseColorType`], if valid.
    pub fn to_ase(&self) -> Option<(AseColor, AseColorType)> {
        if self.components.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let color = match self.mode.trim().to_ascii_lowercase().as_str() {
            "rgb" => {
                let &[r, g, b] = self.components.as_slice() else { return None };
                AseColor::Rgb { r, g, b }
            }
            "cmyk" => {
                let &[c, m, y, k] = self.components.as_slice() else { return None };
                AseColor::Cmyk { c, m, y, k }
            }
            "lab" => {
                let &[l, a, b] = self.components.as_slice() else { return None };
                AseColor::Lab { l, a, b }
            }
            "gray" | "grey" => {
                let &[value] = self.components.as_slice() else { return None };
                AseColor::Gray { value }
            }
            _ => return None,
        };
        let kind = match self.color_type.as_deref().map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("spot") => AseColorType::Spot,
            Some("global") => AseColorType::Global,
            _ => AseColorType::Process,
        };
        Some((color, kind))
    }
}

/// One representation entry inside a [`CcElement`] (primary payload, rendition preview, or inline JSON data).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CcRepresentation {
    /// Representation identifier within the element (`"rep-primary"`, `"rep-thumb"`, ...).
    pub id: String,
    /// Representation MIME type (for example `"image/png"`, `"application/vnd.adobe.color+json"`,
    /// `"model/gltf-binary"`).
    #[serde(rename = "type")]
    pub representation_type: String,
    /// Role of this representation (`"primary"`, `"rendition"`, `"thumbnail"`).
    #[serde(default = "default_relationship")]
    pub relationship: String,
    /// Optional relative asset path inside the `.cclibs` ZIP archive (`"elements/<id>/primary.png"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional inline JSON data (such as `color#data`, `colortheme#data`, `characterstyle#data`,
    /// or `agc+json` descriptors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

fn default_relationship() -> String {
    "primary".to_string()
}

impl CcRepresentation {
    /// Create an inline JSON representation (no external binary file needed).
    pub fn inline_json(id: impl Into<String>, representation_type: impl Into<String>, value: serde_json::Value) -> Self {
        Self { id: id.into(), representation_type: representation_type.into(), relationship: "primary".to_string(), path: None, value: Some(value) }
    }

    /// Create a file-backed asset representation pointing to `path` inside the `.cclibs` archive.
    pub fn asset_file(
        id: impl Into<String>,
        representation_type: impl Into<String>,
        relationship: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            representation_type: representation_type.into(),
            relationship: relationship.into(),
            path: Some(path.into()),
            value: None,
        }
    }
}

/// A single design asset element stored in a Creative Cloud Library (`.cclibs`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CcElement {
    /// Unique element identifier within the library.
    pub id: String,
    /// User-visible element name.
    pub name: String,
    /// Canonical `LIBS v4.20.2` element MIME type.
    #[serde(rename = "type")]
    pub element_type: CcElementType,
    /// Relative logical path in the DCX manifest (`"elements/<id>"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional library group/folder identifier this element belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// Representations of this element (inline JSON values and/or archive asset files).
    #[serde(default)]
    pub representations: Vec<CcRepresentation>,
    /// Additional extensible element metadata preserved across round-trips.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl CcElement {
    /// Create a new element with the given `id`, `name`, and [`CcElementType`].
    pub fn new(id: impl Into<String>, name: impl Into<String>, element_type: CcElementType) -> Self {
        let id_str = id.into();
        let path = Some(format!("elements/{id_str}"));
        Self { id: id_str, name: name.into(), element_type, path, group_id: None, representations: Vec::new(), metadata: BTreeMap::new() }
    }

    /// Build a `color+dcx` element from an [`AseColor`] and [`AseColorType`].
    pub fn from_color(id: impl Into<String>, name: impl Into<String>, color: AseColor, color_type: AseColorType) -> Self {
        let mut elem = Self::new(id, name, CcElementType::Color);
        if let Ok(val) = serde_json::to_value(CcColorValue::from_ase(color, color_type)) {
            elem.representations.push(CcRepresentation::inline_json("color-data", "application/vnd.adobe.color+json", val));
        }
        elem
    }

    /// Build a `colortheme+dcx` element from a slice of `(AseColor, AseColorType)` swatches.
    pub fn from_color_theme(id: impl Into<String>, name: impl Into<String>, swatches: &[(AseColor, AseColorType)]) -> Self {
        let mut elem = Self::new(id, name, CcElementType::ColorTheme);
        let values: Vec<CcColorValue> = swatches.iter().map(|&(c, t)| CcColorValue::from_ase(c, t)).collect();
        if let Ok(val) = serde_json::to_value(values) {
            elem.representations.push(CcRepresentation::inline_json("colortheme-data", "application/vnd.adobe.colortheme+json", val));
        }
        elem
    }

    /// Extract a single [`CcColorValue`] if this is a `Color` element with inline JSON color data.
    pub fn extract_color(&self) -> Option<CcColorValue> {
        if self.element_type != CcElementType::Color {
            return None;
        }
        self.representations.iter().filter_map(|rep| rep.value.as_ref()).find_map(|v| serde_json::from_value::<CcColorValue>(v.clone()).ok())
    }

    /// Extract theme [`CcColorValue`] entries if this is a `ColorTheme` element.
    pub fn extract_color_theme(&self) -> Option<Vec<CcColorValue>> {
        if self.element_type != CcElementType::ColorTheme {
            return None;
        }
        self.representations.iter().filter_map(|rep| rep.value.as_ref()).find_map(|v| serde_json::from_value::<Vec<CcColorValue>>(v.clone()).ok())
    }
}
