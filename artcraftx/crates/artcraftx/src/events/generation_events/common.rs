use serde::Serialize;

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationServiceProvider {
  Artcraft,
  Fal,
  Grok,
  Higgsfield,
  Midjourney,
  WorldLabs,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationModel {
  #[serde(rename = "flux_1_dev")]
  Flux1Dev,
  #[serde(rename = "flux_1_schnell")]
  Flux1Schnell,
  #[serde(rename = "flux_dev_juggernaut")]
  FluxDevJuggernaut,
  #[serde(rename = "flux_pro_1")]
  FluxPro1,
  #[serde(rename = "flux_pro_1.1")]
  FluxPro11,
  #[serde(rename = "flux_pro_1.1_ultra")]
  FluxPro11Ultra,
  
  #[serde(rename = "flux_pro_kontext_max")]
  FluxProKontextMax,
  #[serde(rename = "gpt_image_1")]
  GptImage1,
  #[serde(rename = "gpt_image_1p5")]
  GptImage1p5,
  #[serde(rename = "gpt_image_2")]
  GptImage2,
  #[serde(rename = "grok_image")]
  GrokImage,
  #[serde(rename = "gemini_25_flash")]
  Gemini25Flash,
  #[serde(rename = "nano_banana")]
  NanoBanana,
  #[serde(rename = "nano_banana_2")]
  NanoBanana2,
  #[serde(rename = "nano_banana_2_lite")]
  NanoBanana2Lite,
  #[serde(rename = "nano_banana_pro")]
  NanoBananaPro,
  #[serde(rename = "seedream_4")]
  Seedream4,
  #[serde(rename = "seedream_4p5")]
  Seedream4p5,
  #[serde(rename = "seedream_5_lite")]
  Seedream5Lite,
  #[serde(rename = "seedream_5p0_pro")]
  Seedream5p0Pro,
  #[serde(rename = "seedream_5p0_pro_u")]
  Seedream5p0ProUltra,

  #[serde(rename = "qwen_edit_2511_angles")]
  QwenEdit2511Angles,
  #[serde(rename = "flux_2_lora_angles")]
  Flux2LoraAngles,

  #[serde(rename = "hunyuan_3d_2_0")]
  Hunyuan3d2_0,
  #[serde(rename = "hunyuan_3d_2_1")]
  Hunyuan3d2_1,
  #[serde(rename = "hunyuan_3d_3")]
  Hunyuan3d3,
  #[serde(rename = "hunyuan_3d_3_sketch")]
  Hunyuan3d3Sketch,
  #[serde(rename = "hunyuan_3d_3p1_pro")]
  Hunyuan3d3p1Pro,
  #[serde(rename = "hunyuan_3d_3p1_rapid")]
  Hunyuan3d3p1Rapid,
  #[serde(rename = "hunyuan_3d_3p1_part")]
  Hunyuan3d3p1Part,
  #[serde(rename = "hunyuan_3d_3p1_topology")]
  Hunyuan3d3p1SmartTopology,
  #[serde(rename = "tripo3d_h3p1")]
  Tripo3dH3p1,
  #[serde(rename = "meshy_v6")]
  MeshyV6,
  #[serde(rename = "rodin_2p5_fast")]
  Rodin2p5Fast,

  #[serde(rename = "worldlabs_marble")]
  WorldlabsMarble,
  #[serde(rename = "worldlabs_marble_0p1_mini")]
  WorldlabsMarble0p1Mini,
  #[serde(rename = "worldlabs_marble_0p1_plus")]
  WorldlabsMarble0p1Plus,
  #[serde(rename = "marble_1p0")]
  Marble1p0,
  #[serde(rename = "marble_1p0_draft")]
  Marble1p0Draft,
  #[serde(rename = "marble_1p1")]
  Marble1p1,
  #[serde(rename = "marble_1p1_plus")]
  Marble1p1Plus,
  #[serde(rename = "triposplat")]
  TripoSplat,

  // Audio generation models
  #[serde(rename = "suno_music")]
  SunoMusic,
  #[serde(rename = "suno_remix")]
  SunoRemix,
  #[serde(rename = "suno_sounds")]
  SunoSounds,
  #[serde(rename = "suno_sample")]
  SunoSample,
  #[serde(rename = "seed_audio_1p0")]
  SeedAudio1p0,

  // Generic Midjourney model, version unknown.
  #[serde(rename = "midjourney")]
  Midjourney,
  #[serde(rename = "midjourney_7")]
  Midjourney7,
  #[serde(rename = "midjourney_7_niji")]
  Midjourney7Niji,
  #[serde(rename = "midjourney_8")]
  Midjourney8,

  // Generic Grok video model, version unknown.
  #[serde(rename = "grok_video")]
  GrokVideo,
  #[serde(rename = "grok_imagine_video_1p5")]
  GrokImagineVideo1p5,
  // TODO: Should be Kling16Pro
  #[serde(rename = "kling_1.6")]
  Kling1_6,
  #[serde(rename = "kling_2.0")]
  Kling2_0,
  #[serde(rename = "kling_2.1_master")]
  Kling21Master,
  #[serde(rename = "kling_2.1_pro")]
  Kling21Pro,
  #[serde(rename = "kling_2p5_turbo_pro")]
  Kling2p5TurboPro,
  #[serde(rename = "kling_2p6_pro")]
  Kling2p6Pro,
  #[serde(rename = "kling_3p0_standard")]
  Kling3p0Standard,
  #[serde(rename = "kling_3p0_pro")]
  Kling3p0Pro,
  #[serde(rename = "happy_horse_1p0")]
  HappyHorse1p0,
  #[serde(rename = "recraft_3")]
  Recraft3,
  #[serde(rename = "sora_2")]
  Sora2,
  #[serde(rename = "sora_2_pro")]
  Sora2Pro,
  #[serde(rename = "seedance_1.0_lite")]
  Seedance10Lite,
  #[serde(rename = "seedance_1p5_pro")]
  Seedance1p5Pro,
  #[serde(rename = "seedance_2p0")]
  Seedance2p0,
  #[serde(rename = "seedance_2p0_fast")]
  Seedance2p0Fast,
  #[serde(rename = "seedance_2p0_mini")]
  Seedance2p0Mini,
  #[serde(rename = "seedance_2p5")]
  Seedance2p5,
  #[serde(rename = "seedance_2p5_edit")]
  Seedance2p5Edit,
  #[serde(rename = "minimax_h3")]
  MinimaxH3,
  #[serde(rename = "veo_2")]
  Veo2,
  #[serde(rename = "veo_3")]
  Veo3,
  #[serde(rename = "veo_3_fast")]
  Veo3Fast,
  #[serde(rename = "veo_3p1")]
  Veo3p1,
  #[serde(rename = "veo_3p1_fast")]
  Veo3p1Fast,
  #[serde(rename = "veo_3p1_lite")]
  Veo3p1Lite,
  #[serde(rename = "vidu_q3")]
  ViduQ3,
  #[serde(rename = "vidu_q3_turbo")]
  ViduQ3Turbo,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationAction {
  GenerateImage,
  GenerateVideo,
  GenerateAudio,
  #[serde(rename = "image_to_3d")]
  ImageTo3d,
  GenerateGaussian,
}
