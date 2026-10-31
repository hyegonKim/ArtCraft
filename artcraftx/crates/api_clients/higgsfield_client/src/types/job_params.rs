use crate::types::image_quality::ImageQuality;
use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::image_resolution::ImageResolution;
use crate::types::image_seed::ImageSeed;
use crate::types::media_input::MediaInput;
use crate::types::media_reference::MediaReference;
use crate::types::thinking_level::ThinkingLevel;
use crate::types::video_bitrate_mode::VideoBitrateMode;
use crate::types::video_mode::VideoMode;
use serde::Deserialize;
use serde_json::Value;

/// The server's normalized view of a job's parameters, as echoed on job sets
/// and job status responses.
///
/// Every pipeline has its own shape (nano banana has `input_images`, GPT
/// image has `medias` / `quality` / `model`, ...), so only the fields common
/// to image jobs are typed; everything else stays available in `extra`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct JobParams {
  #[serde(default)]
  pub prompt: Option<String>,

  #[serde(default)]
  pub aspect_ratio: Option<ImageAspectRatio>,

  #[serde(default)]
  pub resolution: Option<ImageResolution>,

  /// The server-derived pixel size (can differ from what was requested).
  #[serde(default)]
  pub width: Option<u32>,

  #[serde(default)]
  pub height: Option<u32>,

  #[serde(default)]
  pub batch_size: Option<u32>,

  /// GPT Image (low/medium/high) and Seedream lite / 4.5 (basic/high/ultra).
  #[serde(default)]
  pub quality: Option<ImageQuality>,

  /// Seedream models only.
  #[serde(default)]
  pub seed: Option<ImageSeed>,

  /// Nano Banana 2 Lite only.
  #[serde(default)]
  pub thinking: Option<ThinkingLevel>,

  // ── Video ──

  /// Clip length in whole seconds.
  #[serde(default)]
  pub duration: Option<u32>,

  /// Seedance: whether a soundtrack was generated.
  #[serde(default)]
  pub generate_audio: Option<bool>,

  /// Seedance 2.x: output bitrate tier.
  #[serde(default)]
  pub bitrate_mode: Option<VideoBitrateMode>,

  /// Seedance 2.0 / Kling: the quality mode (`std` / `pro` / `4k`).
  #[serde(default)]
  pub mode: Option<VideoMode>,

  /// GPT Image only: the sub-model actually used (e.g. `videotape-alpha`).
  #[serde(default)]
  pub model: Option<String>,

  // ── References ──

  /// Reference media with roles (most models). Empty for text-only jobs.
  #[serde(default)]
  pub medias: Vec<MediaReference>,

  /// Reference images on the two models that use this older field instead
  /// of `medias` (Nano Banana Pro, Seedream 4.5).
  #[serde(default)]
  pub input_images: Vec<MediaInput>,

  /// Everything not typed above, keyed by field name.
  #[serde(flatten)]
  pub extra: serde_json::Map<String, Value>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_role::MediaRole;

  #[test]
  fn nano_banana_params_parse() {
    let json = r#"{"width":864,"height":1184,"aspect_ratio":"3:4","resolution":"1k","batch_size":1,"input_images":[],"input_image":null,"application":null,"surface":null,"prompt":"a dinosaur on a skateboard"}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.prompt.as_deref(), Some("a dinosaur on a skateboard"));
    assert_eq!(params.aspect_ratio, Some(ImageAspectRatio::Portrait3x4));
    assert_eq!(params.resolution, Some(ImageResolution::OneK));
    assert_eq!((params.width, params.height), (Some(864), Some(1184)));
    assert_eq!(params.batch_size, Some(1));
    assert!(params.quality.is_none());
    assert!(params.input_images.is_empty());
    assert!(!params.extra.contains_key("input_images"));
  }

  #[test]
  fn reference_media_parse_with_roles() {
    let json = r#"{"width":854,"height":480,"prompt":"p","medias":[{"role":"start_image","data":{"id":"00000000-0000-4000-8000-000000000001","type":"media_input","url":"https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000001.png"}}],"duration":1,"resolution":"480p","aspect_ratio":"auto"}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.medias.len(), 1);
    assert_eq!(params.medias[0].role, MediaRole::StartImage);
    assert_eq!(params.medias[0].data.id.as_str(), "00000000-0000-4000-8000-000000000001");
    assert!(!params.extra.contains_key("medias"));
  }

  #[test]
  fn input_images_parse() {
    let json = r#"{"width":1344,"height":768,"aspect_ratio":"16:9","resolution":"1k","batch_size":1,"input_images":[{"id":"00000000-0000-4000-8000-000000000002","type":"media_input","url":"https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000002.png"}],"input_image":null,"application":null,"surface":null,"prompt":"a corgi on a bike"}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.input_images, vec![MediaInput::uploaded("00000000-0000-4000-8000-000000000002", "https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000002.png")]);
  }

  #[test]
  fn seedream_lite_params_parse() {
    let json = r#"{"prompt":"a corgi on a bike","medias":[],"batch_size":1,"aspect_ratio":"3:4","width":1728,"height":2304,"quality":"basic","seed":12745,"reference_elements":[]}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.quality, Some(ImageQuality::Basic));
    assert_eq!(params.seed, Some(ImageSeed::new(12745)));
    assert!(params.resolution.is_none());
  }

  #[test]
  fn nano_banana_2_lite_params_parse() {
    let json = r#"{"width":864,"height":1184,"aspect_ratio":"3:4","batch_size":1,"thinking":"MINIMAL","is_inpaint":false,"prompt":"a corgi on a bike","medias":[],"reference_elements":[]}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.thinking, Some(ThinkingLevel::Minimal));
    assert_eq!(params.extra.get("is_inpaint"), Some(&Value::Bool(false)));
  }

  #[test]
  fn seedance_video_params_parse() {
    let json = r#"{"width":854,"height":480,"prompt":"a shiba inu skateboarding down a hill","genre":"auto","medias":[],"duration":4,"resolution":"480p","aspect_ratio":"16:9","generate_audio":true,"multi_shots":false,"multi_shot_mode":"custom","multi_prompt":[],"speedramp":"auto","reference_elements":[],"prompt_language":"en","model":"default","extension_mode":null,"bitrate_mode":"high"}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.duration, Some(4));
    assert_eq!(params.generate_audio, Some(true));
    assert_eq!(params.bitrate_mode, Some(VideoBitrateMode::High));
    assert_eq!(params.resolution, Some(ImageResolution::Other("480p".to_string())));
    assert_eq!(params.extra.get("speedramp"), Some(&Value::String("auto".to_string())));
  }

  #[test]
  fn kling_video_params_parse() {
    let json = r#"{"width":1280,"height":720,"prompt":"p","medias":[],"duration":3,"aspect_ratio":"16:9","multi_shots":false,"multi_prompt":[],"sound":"on","cfg_scale":0.5,"mode":"std","kling_elements":[],"kling_element_ids":[],"multi_shot_mode":"auto","reference_elements":[],"enhance_prompt":true}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.mode, Some(VideoMode::Std));
    assert_eq!(params.duration, Some(3));
  }

  #[test]
  fn gpt_image_params_parse() {
    let json = r#"{"width":1152,"height":2048,"prompt":"a corgi on a bike","medias":[],"aspect_ratio":"9:16","quality":"high","resolution":"2k","model":"videotape-alpha","remove_bg":false,"reference_elements":[]}"#;
    let params: JobParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.quality, Some(ImageQuality::High));
    assert_eq!(params.model.as_deref(), Some("videotape-alpha"));
    assert_eq!(params.extra.get("remove_bg"), Some(&Value::Bool(false)));
  }
}
