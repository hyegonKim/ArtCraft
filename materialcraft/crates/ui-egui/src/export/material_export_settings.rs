use serde::{Deserialize, Serialize};

/// MaterialX and .mcmat publishing options from #190, kept for the exporter that will read them
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MaterialExportSettings {
    /// Folder the MaterialX document and its resources go to
    pub folder: String,
    /// The .mcmat archive path
    pub archive: String,
    /// Pixels of the larger side; 0 keeps the graph's size
    pub size: u32,
    pub texture_depth: TextureDepth,
    pub normal_orientation: NormalOrientation,
    /// Height in scene units of a white displacement texel
    pub displacement_scale: f32,
    /// Add preview.png next to material.mtlx
    pub include_preview: bool,
    pub controls: ExposedControls,
}

impl Default for MaterialExportSettings {
    fn default() -> Self {
        Self {
            folder: String::new(),
            archive: String::new(),
            size: 0,
            texture_depth: TextureDepth::default(),
            normal_orientation: NormalOrientation::default(),
            displacement_scale: 1.0,
            include_preview: false,
            controls: ExposedControls::default(),
        }
    }
}

/// PNG precision of the published textures; HDR and signed data always use lossless EXR
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureDepth {
    /// 16 bits for normal and height, 8 bits for the rest
    #[default]
    Automatic,
    Eight,
    Sixteen,
}

impl TextureDepth {
    pub const ALL: [TextureDepth; 3] = [TextureDepth::Automatic, TextureDepth::Eight, TextureDepth::Sixteen];

    pub fn label(self) -> &'static str {
        match self {
            TextureDepth::Automatic => "Automatic (16 bits for normal and height)",
            TextureDepth::Eight => "8 bits",
            TextureDepth::Sixteen => "16 bits",
        }
    }
}

/// Which way the green channel of published normal maps points
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NormalOrientation {
    #[default]
    OpenGl,
    DirectX,
}

impl NormalOrientation {
    pub const ALL: [NormalOrientation; 2] = [NormalOrientation::OpenGl, NormalOrientation::DirectX];

    pub fn label(self) -> &'static str {
        match self {
            NormalOrientation::OpenGl => "OpenGL (Y+)",
            NormalOrientation::DirectX => "DirectX (Y-)",
        }
    }
}

/// What happens to the graph's exposed parameters in the published material
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExposedControls {
    /// Keep the ones the published material can still evaluate
    #[default]
    KeepSupported,
    /// Bake every control into the textures
    BakeAll,
}

impl ExposedControls {
    pub const ALL: [ExposedControls; 2] = [ExposedControls::KeepSupported, ExposedControls::BakeAll];

    pub fn label(self) -> &'static str {
        match self {
            ExposedControls::KeepSupported => "Keep supported controls",
            ExposedControls::BakeAll => "Bake every control",
        }
    }
}
