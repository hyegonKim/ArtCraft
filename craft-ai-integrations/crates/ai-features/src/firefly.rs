//! Multi-modal Creative Cloud AI task taxonomy, Craft Suite host-app routing matrix,
//! local vs. cloud execution capabilities, generative credit estimator, and C2PA Content
//! Credentials provenance metadata generation.
//!
//! Pure data and deterministic calculations (`serde` + `craft-ai-core` only) so every host app,
//! CLI tool, and WebAssembly target shares one clean-room specification.

use craft_ai_core::ids::IdError;
use craft_ai_core::{AppId, CapabilityId, InputRole, Modality, Role, RoleId};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// IPTC Digital Source Type URI for media created purely by a trained generative AI model.
pub const IPTC_TRAINED_ALGORITHMIC_MEDIA: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

/// IPTC Digital Source Type URI for media composited, inpainted, extended, or dubbed with
/// trained generative AI.
pub const IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia";

/// IPTC Digital Source Type URI for media processed by non-generative ML (such as matting or
/// speech denoising).
pub const IPTC_ALGORITHMICALLY_ENHANCED: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/algorithmicallyEnhanced";

/// C2PA standard action identifier for newly generated media assets.
pub const C2PA_ACTION_CREATED: &str = "c2pa.created";

/// C2PA standard action identifier for modifying, inpainting, outpainting, matting, or
/// enhancing existing media.
pub const C2PA_ACTION_EDITED: &str = "c2pa.edited";

/// C2PA standard action identifier for generative vector/image color adjustments.
pub const C2PA_ACTION_COLOR_ADJUSTMENTS: &str = "c2pa.color_adjustments";

/// C2PA standard action identifier for translated audio dubbing.
pub const C2PA_ACTION_DUBBED: &str = "c2pa.dubbed";

/// Standard image pixel ceiling (2048 x 2048 = 4,194,304 px) for 1-credit raster generations.
pub const STANDARD_IMAGE_MAX_PIXELS: u64 = 2048 * 2048;

/// High-resolution 4K pixel ceiling (4096 x 4096 = 16,777,216 px) for 2-credit raster generations.
pub const HIGH_RES_IMAGE_MAX_PIXELS: u64 = 4096 * 4096;

/// 720p video frame pixel ceiling (1280 x 720 = 921,600 px).
pub const VIDEO_720P_MAX_PIXELS: u64 = 1280 * 720;

/// 1080p video frame pixel ceiling (1920 x 1080 = 2,073,600 px).
pub const VIDEO_1080P_MAX_PIXELS: u64 = 1920 * 1080;

/// Maximumvariations cap applied during credit estimation so untrusted inputs cannot overflow
/// downstream quota displays.
pub const MAX_ESTIMATED_VARIATIONS: u32 = 64;

/// Maximum clip duration in seconds (4 hours) considered in a single credit estimate.
pub const MAX_ESTIMATED_DURATION_SECS: u64 = 4 * 3600;

/// Twelve multi-modal Creative Cloud Firefly and Sensei workflows supported across the Craft Suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreativeAiTask {
    /// Masked inpainting that blends generated pixels into a selected region.
    GenerativeFill,
    /// Canvas outpainting that extends borders to match existing scene context.
    GenerativeExpand,
    /// Prompt-driven raster image synthesis from scratch.
    TextToImage,
    /// Image synthesis guided by edge, depth, or layout structure from a reference image.
    StructureReference,
    /// Image synthesis guided by palette, brushwork, or visual style from a reference image.
    StyleReference,
    /// Prompt-driven editable vector scene, icon, or pattern synthesis.
    TextToVectorGraphic,
    /// Prompt-guided palette and harmony recoloring of vector artwork.
    GenerativeRecolor,
    /// Vector-shape-bounded generative fill tailored to custom path silhouettes.
    GenerativeShapeFill,
    /// Temporal shot extension adding generated frames and ambient audio to a clip.
    GenerativeExtendVideo,
    /// Alpha matting and subject isolation removing the background behind the main subject.
    RemoveBackground,
    /// Speech denoising, dereverberation, and studio-clarity vocal enhancement.
    EnhanceSpeech,
    /// Cross-lingual voice translation and timing-aligned audio dubbing.
    TranslateAudioDub,
}

impl CreativeAiTask {
    /// Every task in canonical declaration order.
    pub const ALL: [Self; 12] = [
        Self::GenerativeFill,
        Self::GenerativeExpand,
        Self::TextToImage,
        Self::StructureReference,
        Self::StyleReference,
        Self::TextToVectorGraphic,
        Self::GenerativeRecolor,
        Self::GenerativeShapeFill,
        Self::GenerativeExtendVideo,
        Self::RemoveBackground,
        Self::EnhanceSpeech,
        Self::TranslateAudioDub,
    ];

    /// Stable kebab-case identifier used in role bindings and telemetry-free logs.
    pub const fn id(self) -> &'static str {
        match self {
            Self::GenerativeFill => "generative-fill",
            Self::GenerativeExpand => "generative-expand",
            Self::TextToImage => "text-to-image",
            Self::StructureReference => "structure-reference",
            Self::StyleReference => "style-reference",
            Self::TextToVectorGraphic => "text-to-vector-graphic",
            Self::GenerativeRecolor => "generative-recolor",
            Self::GenerativeShapeFill => "generative-shape-fill",
            Self::GenerativeExtendVideo => "generative-extend-video",
            Self::RemoveBackground => "remove-background",
            Self::EnhanceSpeech => "enhance-speech",
            Self::TranslateAudioDub => "translate-audio-dub",
        }
    }

    /// Parses a task from either kebab-case (`generative-fill`) or snake_case (`generative_fill`).
    pub fn from_id(id: &str) -> Option<Self> {
        let normalized = id.trim().to_ascii_lowercase().replace('_', "-");
        Self::ALL.into_iter().find(|t| t.id() == normalized)
    }

    /// Human-readable English label for UI surfaces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::GenerativeFill => "Generative Fill",
            Self::GenerativeExpand => "Generative Expand",
            Self::TextToImage => "Text to Image",
            Self::StructureReference => "Structure Reference",
            Self::StyleReference => "Style Reference",
            Self::TextToVectorGraphic => "Text to Vector Graphic",
            Self::GenerativeRecolor => "Generative Recolor",
            Self::GenerativeShapeFill => "Generative Shape Fill",
            Self::GenerativeExtendVideo => "Generative Extend Video",
            Self::RemoveBackground => "Remove Background",
            Self::EnhanceSpeech => "Enhance Speech",
            Self::TranslateAudioDub => "Translate Audio Dub",
        }
    }

    /// Primary data modality the workflow operates on.
    pub const fn modality(self) -> Modality {
        match self {
            Self::GenerativeFill
            | Self::GenerativeExpand
            | Self::TextToImage
            | Self::StructureReference
            | Self::StyleReference
            | Self::GenerativeShapeFill
            | Self::RemoveBackground => Modality::Image,
            Self::TextToVectorGraphic | Self::GenerativeRecolor => Modality::Vector,
            Self::GenerativeExtendVideo => Modality::Video,
            Self::EnhanceSpeech | Self::TranslateAudioDub => Modality::Audio,
        }
    }

    const fn capability_task(self) -> &'static str {
        match self {
            Self::GenerativeFill | Self::GenerativeShapeFill => "inpaint",
            Self::GenerativeExpand => "outpaint",
            Self::TextToImage
            | Self::StructureReference
            | Self::StyleReference
            | Self::TextToVectorGraphic
            | Self::GenerativeExtendVideo => "generate",
            Self::GenerativeRecolor => "recolor",
            Self::RemoveBackground => "matte",
            Self::EnhanceSpeech => "enhance",
            Self::TranslateAudioDub => "dub",
        }
    }

    /// Shared `craft-ai-core` `<modality>.<task>` capability identifier.
    pub fn capability_id(self) -> Result<CapabilityId, IdError> {
        CapabilityId::new(&self.modality(), self.capability_task())
    }

    /// Input roles a provider must accept to run this workflow.
    pub fn required_inputs(self) -> Vec<InputRole> {
        match self {
            Self::GenerativeFill | Self::GenerativeShapeFill => {
                vec![InputRole::Source, InputRole::Mask, InputRole::Prompt]
            }
            Self::GenerativeExpand | Self::GenerativeRecolor | Self::GenerativeExtendVideo => {
                vec![InputRole::Source, InputRole::Prompt]
            }
            Self::TextToImage | Self::TextToVectorGraphic => vec![InputRole::Prompt],
            Self::StructureReference | Self::StyleReference => {
                vec![InputRole::Prompt, InputRole::Reference]
            }
            Self::RemoveBackground | Self::EnhanceSpeech | Self::TranslateAudioDub => {
                vec![InputRole::Source]
            }
        }
    }

    /// Builds a `craft_ai_core::Role` descriptor for binding this task to a model connection.
    pub fn role(self) -> Result<Role, IdError> {
        Ok(Role {
            id: RoleId::new(self.id())?,
            label: self.label().to_owned(),
            capability: self.capability_id()?,
            inputs: self.required_inputs(),
        })
    }

    /// Craft Suite host applications where this task is surfaced.
    pub const fn supported_hosts(self) -> &'static [CraftHostApp] {
        use CraftHostApp::{
            ArtCraft, DeckCraft, DesignCraft, EffectCraft, FilmCraft, LightCraft, PhotoCraft,
            SoundCraft, VectorCraft,
        };
        match self {
            Self::GenerativeFill => &[PhotoCraft, LightCraft, DesignCraft, ArtCraft],
            Self::GenerativeExpand => &[PhotoCraft, LightCraft, DesignCraft, DeckCraft, ArtCraft],
            Self::TextToImage => &[PhotoCraft, DesignCraft, DeckCraft, VectorCraft, ArtCraft],
            Self::StructureReference => &[PhotoCraft, DesignCraft, DeckCraft, ArtCraft],
            Self::StyleReference => &[PhotoCraft, DesignCraft, DeckCraft, ArtCraft],
            Self::TextToVectorGraphic => &[VectorCraft, DesignCraft, DeckCraft, ArtCraft],
            Self::GenerativeRecolor => &[VectorCraft, DesignCraft, ArtCraft],
            Self::GenerativeShapeFill => &[VectorCraft, PhotoCraft, DesignCraft, ArtCraft],
            Self::GenerativeExtendVideo => &[FilmCraft, EffectCraft, ArtCraft],
            Self::RemoveBackground => &[
                PhotoCraft,
                LightCraft,
                DesignCraft,
                DeckCraft,
                VectorCraft,
                ArtCraft,
            ],
            Self::EnhanceSpeech => &[SoundCraft, FilmCraft, EffectCraft, ArtCraft],
            Self::TranslateAudioDub => &[FilmCraft, SoundCraft, EffectCraft, DeckCraft, ArtCraft],
        }
    }

    /// Returns `true` when `host` supports running this task.
    pub fn supports_host(self, host: CraftHostApp) -> bool {
        self.supported_hosts().contains(&host)
    }

    /// Local vs. cloud execution capability for this workflow.
    pub fn execution_capability(self) -> ExecutionCapability {
        match self {
            Self::RemoveBackground => ExecutionCapability::LocalCapable {
                engines: vec![LocalEngine::Burn, LocalEngine::Rten],
                cloud_presets: vec!["artcraft-omni".into()],
            },
            Self::EnhanceSpeech => ExecutionCapability::LocalCapable {
                engines: vec![LocalEngine::Rten, LocalEngine::Candle],
                cloud_presets: vec!["openai".into(), "artcraft-omni".into()],
            },
            Self::GenerativeFill | Self::GenerativeShapeFill => {
                ExecutionCapability::CloudProvider {
                    presets: vec![
                        "openai".into(),
                        "bfl".into(),
                        "gemini".into(),
                        "artcraft-omni".into(),
                    ],
                }
            }
            Self::GenerativeExpand => ExecutionCapability::CloudProvider {
                presets: vec!["openai".into(), "bfl".into(), "artcraft-omni".into()],
            },
            Self::TextToImage => ExecutionCapability::CloudProvider {
                presets: vec![
                    "openai".into(),
                    "bfl".into(),
                    "vertex-express".into(),
                    "artcraft-omni".into(),
                ],
            },
            Self::StructureReference | Self::StyleReference => ExecutionCapability::CloudProvider {
                presets: vec![
                    "bfl".into(),
                    "openai".into(),
                    "gemini".into(),
                    "artcraft-omni".into(),
                ],
            },
            Self::TextToVectorGraphic | Self::GenerativeRecolor => {
                ExecutionCapability::CloudProvider {
                    presets: vec!["openrouter".into(), "openai".into(), "artcraft-omni".into()],
                }
            }
            Self::GenerativeExtendVideo => ExecutionCapability::CloudProvider {
                presets: vec!["bfl".into(), "seedance".into(), "artcraft-omni".into()],
            },
            Self::TranslateAudioDub => ExecutionCapability::CloudProvider {
                presets: vec!["openai".into(), "openrouter".into(), "artcraft-omni".into()],
            },
        }
    }

    /// Default C2PA Content Credentials provenance tag for this workflow.
    pub fn c2pa_provenance(self) -> C2paProvenance {
        generate_c2pa_provenance(self, None, None)
    }
}

impl std::fmt::Display for CreativeAiTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Craft Suite host applications that integrate Creative AI workflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CraftHostApp {
    PhotoCraft,
    VectorCraft,
    FilmCraft,
    EffectCraft,
    LightCraft,
    SoundCraft,
    DesignCraft,
    DeckCraft,
    ArtCraft,
}

impl CraftHostApp {
    /// All nine Craft Suite host applications in canonical order.
    pub const ALL: [Self; 9] = [
        Self::PhotoCraft,
        Self::VectorCraft,
        Self::FilmCraft,
        Self::EffectCraft,
        Self::LightCraft,
        Self::SoundCraft,
        Self::DesignCraft,
        Self::DeckCraft,
        Self::ArtCraft,
    ];

    /// Lowercase identifier matching `craft_ai_core::AppId` conventions.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PhotoCraft => "photocraft",
            Self::VectorCraft => "vectorcraft",
            Self::FilmCraft => "filmcraft",
            Self::EffectCraft => "effectcraft",
            Self::LightCraft => "lightcraft",
            Self::SoundCraft => "soundcraft",
            Self::DesignCraft => "designcraft",
            Self::DeckCraft => "deckcraft",
            Self::ArtCraft => "artcraft",
        }
    }

    /// Display title of the Craft Suite application.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::PhotoCraft => "PhotoCraft",
            Self::VectorCraft => "VectorCraft",
            Self::FilmCraft => "FilmCraft",
            Self::EffectCraft => "EffectCraft",
            Self::LightCraft => "LightCraft",
            Self::SoundCraft => "SoundCraft",
            Self::DesignCraft => "DesignCraft",
            Self::DeckCraft => "DeckCraft",
            Self::ArtCraft => "ArtCraft",
        }
    }

    /// Parses a host app identifier case-insensitively.
    pub fn from_id(s: &str) -> Option<Self> {
        let lower = s.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|app| app.as_str() == lower)
    }

    /// Converts to a validated `craft_ai_core::AppId`.
    pub fn app_id(self) -> Result<AppId, IdError> {
        AppId::new(self.as_str())
    }

    /// Returns every `CreativeAiTask` compatible with this host application.
    pub fn supported_tasks(self) -> Vec<CreativeAiTask> {
        CreativeAiTask::ALL
            .into_iter()
            .filter(|t| t.supports_host(self))
            .collect()
    }
}

impl std::fmt::Display for CraftHostApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Built-in native Rust ML inference engines available without external Python servers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalEngine {
    /// Burn tensor/WGPU/Vulkan/Metal/CPU runtime (`ai-engine-burn`).
    Burn,
    /// Hugging Face Candle CUDA/Metal/CPU runtime (`ai-engine-candle`).
    Candle,
    /// Pure-Rust ONNX runtime (`ai-engine-rten`).
    Rten,
}

impl LocalEngine {
    /// Provider adapter identifier on a saved local connection.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Burn => "burn",
            Self::Candle => "candle",
            Self::Rten => "rten",
        }
    }
}

/// Whether a task can execute locally via built-in Rust engines (`burn`/`candle`/`rten`) or
/// requires a cloud provider connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExecutionCapability {
    /// Runnable on-device via one or more built-in Rust inference runtimes, with optional hosted
    /// cloud presets when local weights are not installed.
    LocalCapable {
        engines: Vec<LocalEngine>,
        cloud_presets: Vec<String>,
    },
    /// Runs on a hosted cloud model provider (matching keys in `craft_ai_core::PRESETS`).
    CloudProvider { presets: Vec<String> },
}

impl ExecutionCapability {
    /// Returns `true` when the task has at least one built-in on-device engine (`burn`, `candle`,
    /// or `rten`).
    pub fn is_local_capable(&self) -> bool {
        matches!(self, Self::LocalCapable { engines, .. } if !engines.is_empty())
    }

    /// Returns `true` if `engine` can run this task locally.
    pub fn supports_local_engine(&self, engine: LocalEngine) -> bool {
        match self {
            Self::LocalCapable { engines, .. } => engines.contains(&engine),
            Self::CloudProvider { .. } => false,
        }
    }

    /// Hosted preset keys from `craft_ai_core::PRESETS` compatible with this task.
    pub fn cloud_presets(&self) -> &[String] {
        match self {
            Self::LocalCapable { cloud_presets, .. } => cloud_presets,
            Self::CloudProvider { presets } => presets,
        }
    }
}

/// Complete capability-matrix row for one `CreativeAiTask`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCapabilityEntry {
    pub task: CreativeAiTask,
    pub capability: CapabilityId,
    pub required_inputs: Vec<InputRole>,
    pub host_apps: Vec<CraftHostApp>,
    pub execution: ExecutionCapability,
    pub c2pa: C2paProvenance,
}

/// Builds the full 12-task Creative Cloud / Craft Suite capability matrix.
pub fn capability_matrix() -> Vec<TaskCapabilityEntry> {
    CreativeAiTask::ALL
        .into_iter()
        .filter_map(|task| {
            let capability = task.capability_id().ok()?;
            Some(TaskCapabilityEntry {
                task,
                capability,
                required_inputs: task.required_inputs(),
                host_apps: task.supported_hosts().to_vec(),
                execution: task.execution_capability(),
                c2pa: task.c2pa_provenance(),
            })
        })
        .collect()
}

/// Workload dimensions for estimating generative credit consumption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreditWorkload {
    /// Output width in pixels (used by raster image and video tasks).
    pub width: u32,
    /// Output height in pixels (used by raster image and video tasks).
    pub height: u32,
    /// Duration in seconds (used by video extension and audio speech/dubbing tasks).
    pub duration_secs: f64,
    /// Frame rate in frames per second (used by video tasks).
    pub fps: u32,
    /// Number of requested variations / candidates in one job (clamped to `1..=64`).
    pub variations: u32,
    /// Prefer local execution (`burn`/`candle`/`rten`) when the task is `LocalCapable`.
    pub prefer_local: bool,
}

impl Default for CreditWorkload {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 1024,
            duration_secs: 5.0,
            fps: 24,
            variations: 1,
            prefer_local: false,
        }
    }
}

impl CreditWorkload {
    /// Workload for raster image generation, inpainting, outpainting, or matting.
    pub fn image(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Self::default()
        }
    }

    /// Workload for vector generation or recoloring with `variations` outputs.
    pub fn vector(variations: u32) -> Self {
        Self {
            variations,
            ..Self::default()
        }
    }

    /// Workload for video generation/extension at `(width, height)`, `duration_secs`, and `fps`.
    pub fn video(width: u32, height: u32, duration_secs: f64, fps: u32) -> Self {
        Self {
            width,
            height,
            duration_secs,
            fps,
            ..Self::default()
        }
    }

    /// Workload for audio enhancement or translated dubbing of `duration_secs`.
    pub fn audio(duration_secs: f64) -> Self {
        Self {
            duration_secs,
            ..Self::default()
        }
    }

    /// Sets the number of output variations requested.
    pub fn with_variations(mut self, variations: u32) -> Self {
        self.variations = variations;
        self
    }

    /// Requests on-device local execution when available (`0` cloud generative credits).
    pub fn with_local(mut self, prefer_local: bool) -> Self {
        self.prefer_local = prefer_local;
        self
    }
}

/// Resolution or media tier used to explain a credit estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditTier {
    LocalOnDevice,
    StandardImage,
    HighResImage4k,
    UltraImage,
    VectorGraphic,
    Video720p,
    Video1080p,
    Video4k,
    AudioPerMinute,
}

/// Result of `estimate_generative_credits`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditEstimate {
    pub task: CreativeAiTask,
    /// Estimated cloud generative credits (`0` when executed locally on-device).
    pub credits: u64,
    /// `true` when `prefer_local` was requested and the task is `LocalCapable`.
    pub executed_locally: bool,
    /// Pricing tier applied to the workload.
    pub tier: CreditTier,
    /// Billable units (variations, seconds, or started minutes).
    pub billable_units: u64,
    /// Plain-language disclosure safe to show in consent dialogs.
    pub summary: String,
}

/// Estimates Creative Cloud / Firefly-style generative credits for `task` and `workload`.
///
/// Uses checked/saturating integer math and clamps hostile dimensions or non-finite durations so
/// untrusted input never panics or overflows.
pub fn estimate_generative_credits(
    task: CreativeAiTask,
    workload: &CreditWorkload,
) -> CreditEstimate {
    let variations = u64::from(workload.variations.clamp(1, MAX_ESTIMATED_VARIATIONS));
    if workload.prefer_local && task.execution_capability().is_local_capable() {
        return CreditEstimate {
            task,
            credits: 0,
            executed_locally: true,
            tier: CreditTier::LocalOnDevice,
            billable_units: variations,
            summary: format!(
                "{} runs locally on this computer (0 generative credits).",
                task.label()
            ),
        };
    }

    match task {
        CreativeAiTask::GenerativeFill
        | CreativeAiTask::GenerativeExpand
        | CreativeAiTask::TextToImage
        | CreativeAiTask::StructureReference
        | CreativeAiTask::StyleReference
        | CreativeAiTask::GenerativeShapeFill
        | CreativeAiTask::RemoveBackground => estimate_image_credits(task, workload, variations),
        CreativeAiTask::TextToVectorGraphic => CreditEstimate {
            task,
            credits: 2_u64.saturating_mul(variations),
            executed_locally: false,
            tier: CreditTier::VectorGraphic,
            billable_units: variations,
            summary: format!(
                "{}: {} generative credits ({} vector variation(s) at 2 credits each).",
                task.label(),
                2_u64.saturating_mul(variations),
                variations
            ),
        },
        CreativeAiTask::GenerativeRecolor => CreditEstimate {
            task,
            credits: variations,
            executed_locally: false,
            tier: CreditTier::VectorGraphic,
            billable_units: variations,
            summary: format!(
                "{}: {} generative credit(s) ({} palette variation(s)).",
                task.label(),
                variations,
                variations
            ),
        },
        CreativeAiTask::GenerativeExtendVideo => estimate_video_credits(task, workload, variations),
        CreativeAiTask::EnhanceSpeech => estimate_audio_credits(task, workload, variations, 1),
        CreativeAiTask::TranslateAudioDub => estimate_audio_credits(task, workload, variations, 5),
    }
}

fn clamped_seconds(duration_secs: f64) -> u64 {
    if !duration_secs.is_finite() || duration_secs <= 0.0 {
        return 1;
    }
    let ceil = duration_secs.ceil();
    if ceil >= MAX_ESTIMATED_DURATION_SECS as f64 {
        MAX_ESTIMATED_DURATION_SECS
    } else {
        (ceil as u64).max(1)
    }
}

fn estimate_image_credits(
    task: CreativeAiTask,
    workload: &CreditWorkload,
    variations: u64,
) -> CreditEstimate {
    let w = u64::from(workload.width.max(1));
    let h = u64::from(workload.height.max(1));
    let pixels = w.saturating_mul(h);
    let (per_output, tier) = if pixels <= STANDARD_IMAGE_MAX_PIXELS {
        (1_u64, CreditTier::StandardImage)
    } else if pixels <= HIGH_RES_IMAGE_MAX_PIXELS {
        (2_u64, CreditTier::HighResImage4k)
    } else {
        (4_u64, CreditTier::UltraImage)
    };
    let reference_surcharge = u64::from(
        matches!(
            task,
            CreativeAiTask::StructureReference | CreativeAiTask::StyleReference
        ) && pixels > STANDARD_IMAGE_MAX_PIXELS,
    );
    let unit_cost = per_output.saturating_add(reference_surcharge);
    let credits = unit_cost.saturating_mul(variations);
    CreditEstimate {
        task,
        credits,
        executed_locally: false,
        tier,
        billable_units: variations,
        summary: format!(
            "{} ({}x{}): {} generative credit(s) for {} output(s).",
            task.label(),
            w,
            h,
            credits,
            variations
        ),
    }
}

fn estimate_video_credits(
    task: CreativeAiTask,
    workload: &CreditWorkload,
    variations: u64,
) -> CreditEstimate {
    let w = u64::from(workload.width.max(1));
    let h = u64::from(workload.height.max(1));
    let pixels = w.saturating_mul(h);
    let seconds = clamped_seconds(workload.duration_secs);
    let (base_per_sec, tier) = if pixels <= VIDEO_720P_MAX_PIXELS {
        (10_u64, CreditTier::Video720p)
    } else if pixels <= VIDEO_1080P_MAX_PIXELS {
        (20_u64, CreditTier::Video1080p)
    } else {
        (30_u64, CreditTier::Video4k)
    };
    let fps_surcharge = if workload.fps > 30 { 5_u64 } else { 0_u64 };
    let per_sec = base_per_sec.saturating_add(fps_surcharge);
    let billable_units = seconds.saturating_mul(variations);
    let credits = per_sec.saturating_mul(billable_units);
    CreditEstimate {
        task,
        credits,
        executed_locally: false,
        tier,
        billable_units,
        summary: format!(
            "{} ({}s at {}x{}, {} fps): {} generative credits.",
            task.label(),
            seconds,
            w,
            h,
            workload.fps.max(1),
            credits
        ),
    }
}

fn estimate_audio_credits(
    task: CreativeAiTask,
    workload: &CreditWorkload,
    variations: u64,
    credits_per_minute: u64,
) -> CreditEstimate {
    let seconds = clamped_seconds(workload.duration_secs);
    let minutes = seconds.div_ceil(60).max(1);
    let billable_units = minutes.saturating_mul(variations);
    let credits = credits_per_minute.saturating_mul(billable_units);
    CreditEstimate {
        task,
        credits,
        executed_locally: false,
        tier: CreditTier::AudioPerMinute,
        billable_units,
        summary: format!(
            "{} ({} min): {} generative credit(s).",
            task.label(),
            minutes,
            credits
        ),
    }
}

/// C2PA Content Credentials provenance tag pairing a `c2pa.action` identifier with an IPTC
/// `digitalSourceType` URI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paProvenance {
    pub task: CreativeAiTask,
    /// C2PA v2 action identifier (such as `c2pa.created` or `c2pa.edited`).
    pub c2pa_action: String,
    /// IPTC NewsCodes Digital Source Type URI.
    pub digital_source_type: String,
    /// Claim generator / software agent string recorded in the C2PA manifest.
    pub software_agent: String,
    /// Optional model identifier disclosed in action parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// `true` when trained generative AI created or modified content (`trainedAlgorithmicMedia` or
    /// `compositeWithTrainedAlgorithmicMedia`); `false` for non-generative ML enhancement.
    pub is_generative: bool,
}

impl C2paProvenance {
    /// Formats this provenance record as a C2PA `c2pa.actions` v2 action entry JSON object.
    pub fn to_c2pa_action_json(&self) -> Json {
        let mut params = serde_json::Map::new();
        params.insert("craft.task".into(), json!(self.task.id()));
        if let Some(model) = &self.model_id {
            params.insert("model".into(), json!(model));
        }
        json!({
            "action": self.c2pa_action,
            "digitalSourceType": self.digital_source_type,
            "softwareAgent": self.software_agent,
            "parameters": Json::Object(params),
        })
    }
}

/// Generates C2PA Content Credentials provenance metadata (`digitalSourceType` IPTC URI and
/// `c2pa.action` identifier) for `task`, optionally attributed to `host` and `model_id`.
pub fn generate_c2pa_provenance(
    task: CreativeAiTask,
    host: Option<CraftHostApp>,
    model_id: Option<&str>,
) -> C2paProvenance {
    let (c2pa_action, digital_source_type, is_generative) = match task {
        CreativeAiTask::TextToImage
        | CreativeAiTask::StructureReference
        | CreativeAiTask::StyleReference
        | CreativeAiTask::TextToVectorGraphic => {
            (C2PA_ACTION_CREATED, IPTC_TRAINED_ALGORITHMIC_MEDIA, true)
        }
        CreativeAiTask::GenerativeFill
        | CreativeAiTask::GenerativeExpand
        | CreativeAiTask::GenerativeShapeFill
        | CreativeAiTask::GenerativeExtendVideo => (
            C2PA_ACTION_EDITED,
            IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA,
            true,
        ),
        CreativeAiTask::GenerativeRecolor => (
            C2PA_ACTION_COLOR_ADJUSTMENTS,
            IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA,
            true,
        ),
        CreativeAiTask::TranslateAudioDub => (
            C2PA_ACTION_DUBBED,
            IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA,
            true,
        ),
        CreativeAiTask::RemoveBackground | CreativeAiTask::EnhanceSpeech => {
            (C2PA_ACTION_EDITED, IPTC_ALGORITHMICALLY_ENHANCED, false)
        }
    };

    let software_agent = match host {
        Some(app) => format!("Craft Suite / {}", app.display_name()),
        None => "Craft Suite / Craft AI Integrations".to_owned(),
    };

    let clean_model = model_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    C2paProvenance {
        task,
        c2pa_action: c2pa_action.to_owned(),
        digital_source_type: digital_source_type.to_owned(),
        software_agent,
        model_id: clean_model,
        is_generative,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_ai_core::PRESETS;
    use std::collections::HashSet;

    #[test]
    fn all_twelve_tasks_have_unique_ids_labels_roles_and_capabilities() {
        assert_eq!(CreativeAiTask::ALL.len(), 12);
        let mut ids = HashSet::new();
        let mut labels = HashSet::new();
        for task in CreativeAiTask::ALL {
            assert!(ids.insert(task.id()), "duplicate id {}", task.id());
            assert!(
                labels.insert(task.label()),
                "duplicate label {}",
                task.label()
            );
            assert_eq!(CreativeAiTask::from_id(task.id()), Some(task));
            let snake = task.id().replace('-', "_");
            assert_eq!(CreativeAiTask::from_id(&snake), Some(task));
            let cap = task.capability_id().unwrap();
            assert!(!cap.as_str().is_empty());
            let role = task.role().unwrap();
            assert_eq!(role.id.as_str(), task.id());
            assert!(!role.inputs.is_empty());

            let encoded = serde_json::to_string(&task).unwrap();
            let decoded: CreativeAiTask = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, task);
        }
        assert_eq!(CreativeAiTask::from_id("unknown-task"), None);
    }

    #[test]
    fn host_app_capability_matrix_covers_all_nine_craft_apps_and_valid_presets() {
        assert_eq!(CraftHostApp::ALL.len(), 9);
        let matrix = capability_matrix();
        assert_eq!(matrix.len(), 12);

        let valid_presets: HashSet<&str> = PRESETS.iter().map(|p| p.key).collect();
        for entry in &matrix {
            assert!(
                !entry.host_apps.is_empty(),
                "{} must map to at least one host app",
                entry.task.id()
            );
            assert!(
                entry.host_apps.contains(&CraftHostApp::ArtCraft),
                "{} should be routable from ArtCraft suite hub",
                entry.task.id()
            );
            for preset in entry.execution.cloud_presets() {
                assert!(
                    valid_presets.contains(preset.as_str()),
                    "unknown cloud preset {preset} on {}",
                    entry.task.id()
                );
            }
        }

        // Every one of the 9 Craft Suite host apps has domain-appropriate tasks routed to it.
        for host in CraftHostApp::ALL {
            assert_eq!(CraftHostApp::from_id(host.as_str()), Some(host));
            assert!(host.app_id().is_ok());
            let tasks = host.supported_tasks();
            assert!(!tasks.is_empty(), "{host} has no supported tasks");
        }

        // Spot-check domain routing across raster, vector, video, layout, and audio apps.
        assert!(CreativeAiTask::GenerativeFill.supports_host(CraftHostApp::PhotoCraft));
        assert!(!CreativeAiTask::GenerativeFill.supports_host(CraftHostApp::SoundCraft));
        assert!(CreativeAiTask::TextToVectorGraphic.supports_host(CraftHostApp::VectorCraft));
        assert!(CreativeAiTask::GenerativeRecolor.supports_host(CraftHostApp::VectorCraft));
        assert!(CreativeAiTask::GenerativeShapeFill.supports_host(CraftHostApp::VectorCraft));
        assert!(CreativeAiTask::GenerativeExtendVideo.supports_host(CraftHostApp::FilmCraft));
        assert!(CreativeAiTask::GenerativeExtendVideo.supports_host(CraftHostApp::EffectCraft));
        assert!(CreativeAiTask::RemoveBackground.supports_host(CraftHostApp::LightCraft));
        assert!(CreativeAiTask::EnhanceSpeech.supports_host(CraftHostApp::SoundCraft));
        assert!(CreativeAiTask::TranslateAudioDub.supports_host(CraftHostApp::FilmCraft));
        assert!(CreativeAiTask::TranslateAudioDub.supports_host(CraftHostApp::DeckCraft));
        assert!(CreativeAiTask::GenerativeExpand.supports_host(CraftHostApp::DesignCraft));
    }

    #[test]
    fn local_capable_vs_cloud_provider_distinguishes_burn_candle_and_rten() {
        let rm_bg = CreativeAiTask::RemoveBackground.execution_capability();
        assert!(rm_bg.is_local_capable());
        assert!(rm_bg.supports_local_engine(LocalEngine::Burn));
        assert!(rm_bg.supports_local_engine(LocalEngine::Rten));
        assert!(!rm_bg.supports_local_engine(LocalEngine::Candle));

        let speech = CreativeAiTask::EnhanceSpeech.execution_capability();
        assert!(speech.is_local_capable());
        assert!(speech.supports_local_engine(LocalEngine::Candle));
        assert!(speech.supports_local_engine(LocalEngine::Rten));

        for cloud_only in [
            CreativeAiTask::GenerativeFill,
            CreativeAiTask::GenerativeExpand,
            CreativeAiTask::TextToImage,
            CreativeAiTask::StructureReference,
            CreativeAiTask::StyleReference,
            CreativeAiTask::TextToVectorGraphic,
            CreativeAiTask::GenerativeRecolor,
            CreativeAiTask::GenerativeShapeFill,
            CreativeAiTask::GenerativeExtendVideo,
            CreativeAiTask::TranslateAudioDub,
        ] {
            let exec = cloud_only.execution_capability();
            assert!(!exec.is_local_capable(), "{}", cloud_only.id());
            assert!(!exec.supports_local_engine(LocalEngine::Burn));
        }
    }

    #[test]
    fn generative_credit_estimator_scales_with_resolution_duration_variations_and_local_mode() {
        // Local execution of a LocalCapable task costs 0 generative credits.
        let local_bg = estimate_generative_credits(
            CreativeAiTask::RemoveBackground,
            &CreditWorkload::image(4096, 4096).with_local(true),
        );
        assert_eq!(local_bg.credits, 0);
        assert!(local_bg.executed_locally);
        assert_eq!(local_bg.tier, CreditTier::LocalOnDevice);

        // Requesting prefer_local on a CloudProvider-only task still bills cloud credits.
        let cloud_fill = estimate_generative_credits(
            CreativeAiTask::GenerativeFill,
            &CreditWorkload::image(1024, 1024).with_local(true),
        );
        assert_eq!(cloud_fill.credits, 1);
        assert!(!cloud_fill.executed_locally);
        assert_eq!(cloud_fill.tier, CreditTier::StandardImage);

        // 4K image with 4 variations + StructureReference high-res surcharge: (2 + 1) * 4 = 12.
        let struct_4k = estimate_generative_credits(
            CreativeAiTask::StructureReference,
            &CreditWorkload::image(3840, 2160).with_variations(4),
        );
        assert_eq!(struct_4k.tier, CreditTier::HighResImage4k);
        assert_eq!(struct_4k.credits, 12);

        // Ultra-high resolution (>16 MP) TextToImage: 4 credits * 2 variations = 8.
        let ultra = estimate_generative_credits(
            CreativeAiTask::TextToImage,
            &CreditWorkload::image(6000, 4000).with_variations(2),
        );
        assert_eq!(ultra.tier, CreditTier::UltraImage);
        assert_eq!(ultra.credits, 8);

        // Vector tasks.
        let vec_gen = estimate_generative_credits(
            CreativeAiTask::TextToVectorGraphic,
            &CreditWorkload::vector(3),
        );
        assert_eq!(vec_gen.credits, 6);
        let recolor = estimate_generative_credits(
            CreativeAiTask::GenerativeRecolor,
            &CreditWorkload::vector(4),
        );
        assert_eq!(recolor.credits, 4);

        // Video extension: 1080p 5s @ 24fps = 20 * 5 = 100 credits; @ 60fps = 25 * 5 = 125.
        let vid_24 = estimate_generative_credits(
            CreativeAiTask::GenerativeExtendVideo,
            &CreditWorkload::video(1920, 1080, 5.0, 24),
        );
        assert_eq!(vid_24.tier, CreditTier::Video1080p);
        assert_eq!(vid_24.credits, 100);

        let vid_60 = estimate_generative_credits(
            CreativeAiTask::GenerativeExtendVideo,
            &CreditWorkload::video(1920, 1080, 5.0, 60),
        );
        assert_eq!(vid_60.credits, 125);

        // Audio dubbing: 125s -> 3 started minutes * 5 credits/min = 15 credits.
        let dub = estimate_generative_credits(
            CreativeAiTask::TranslateAudioDub,
            &CreditWorkload::audio(125.0),
        );
        assert_eq!(dub.tier, CreditTier::AudioPerMinute);
        assert_eq!(dub.credits, 15);

        // Hostile / extreme inputs never panic or overflow.
        let hostile = CreditWorkload {
            width: u32::MAX,
            height: u32::MAX,
            duration_secs: f64::NAN,
            fps: u32::MAX,
            variations: u32::MAX,
            prefer_local: false,
        };
        for task in CreativeAiTask::ALL {
            let est = estimate_generative_credits(task, &hostile);
            assert!(est.credits > 0);
            assert!(!est.summary.contains('\u{2014}') && !est.summary.contains('\u{2013}'));
        }
    }

    #[test]
    fn c2pa_provenance_generates_valid_iptc_uris_and_c2pa_actions_for_all_tasks() {
        for task in CreativeAiTask::ALL {
            let prov = generate_c2pa_provenance(
                task,
                Some(CraftHostApp::PhotoCraft),
                Some("firefly-image-3"),
            );
            assert!(
                prov.digital_source_type
                    .starts_with("http://cv.iptc.org/newscodes/digitalsourcetype/"),
                "{}: {}",
                task.id(),
                prov.digital_source_type
            );
            assert!(
                prov.c2pa_action.starts_with("c2pa."),
                "{}: {}",
                task.id(),
                prov.c2pa_action
            );
            assert_eq!(prov.software_agent, "Craft Suite / PhotoCraft");
            assert_eq!(prov.model_id.as_deref(), Some("firefly-image-3"));

            let action_json = prov.to_c2pa_action_json();
            assert_eq!(action_json["action"], prov.c2pa_action);
            assert_eq!(action_json["digitalSourceType"], prov.digital_source_type);
            assert_eq!(action_json["parameters"]["craft.task"], task.id());
            assert_eq!(action_json["parameters"]["model"], "firefly-image-3");
        }

        // Verify exact IPTC URI distinctions across pure synthesis, composite generative edit,
        // color adjustment, dubbing, and non-generative ML enhancement.
        let t2i = CreativeAiTask::TextToImage.c2pa_provenance();
        assert_eq!(t2i.c2pa_action, C2PA_ACTION_CREATED);
        assert_eq!(t2i.digital_source_type, IPTC_TRAINED_ALGORITHMIC_MEDIA);
        assert!(t2i.is_generative);

        let fill = CreativeAiTask::GenerativeFill.c2pa_provenance();
        assert_eq!(fill.c2pa_action, C2PA_ACTION_EDITED);
        assert_eq!(
            fill.digital_source_type,
            IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA
        );
        assert!(fill.is_generative);

        let recolor = CreativeAiTask::GenerativeRecolor.c2pa_provenance();
        assert_eq!(recolor.c2pa_action, C2PA_ACTION_COLOR_ADJUSTMENTS);
        assert_eq!(
            recolor.digital_source_type,
            IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA
        );

        let dub = CreativeAiTask::TranslateAudioDub.c2pa_provenance();
        assert_eq!(dub.c2pa_action, C2PA_ACTION_DUBBED);
        assert_eq!(
            dub.digital_source_type,
            IPTC_COMPOSITE_WITH_TRAINED_ALGORITHMIC_MEDIA
        );

        let matte = CreativeAiTask::RemoveBackground.c2pa_provenance();
        assert_eq!(matte.c2pa_action, C2PA_ACTION_EDITED);
        assert_eq!(matte.digital_source_type, IPTC_ALGORITHMICALLY_ENHANCED);
        assert!(!matte.is_generative);
    }

    #[test]
    fn capability_matrix_json_contains_no_em_or_en_dashes() {
        let json = serde_json::to_string(&capability_matrix()).unwrap();
        assert!(!json.contains('\u{2014}') && !json.contains('\u{2013}'));
    }
}
