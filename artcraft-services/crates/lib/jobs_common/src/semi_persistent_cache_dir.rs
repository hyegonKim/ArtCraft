use std::fs;
use std::path::{Path, PathBuf};

use errors::AnyhowResult;

/// These are for files on the worker filesystems
pub struct SemiPersistentCacheDir {
  cache_root: PathBuf,

  tts_synthesizer_model_root: PathBuf,
  tts_pretrained_vocoder_model_root: PathBuf, // Pretrained, non-user uploaded vocoders
  vocoder_model_root: PathBuf, // User-uploaded vocoders
  gpt_sovits_model_root: PathBuf, // GPT-SoViT models

  voice_conversion_model_root: PathBuf,

  w2l_model_root: PathBuf,
  w2l_end_bump_root: PathBuf,
  w2l_face_templates_root: PathBuf,
  w2l_templates_media_root: PathBuf,
  video_asset_root: PathBuf, // end bump, etc.
}

impl SemiPersistentCacheDir {


  /// Everything is rooted at `/file_cache`.
  pub fn default_paths() -> Self {
    // NB: This is the root of the filesystem
    Self::configured_root("/file_cache/")
  }

  pub fn configured_root<P: AsRef<Path>>(root_path: P) -> Self {
    let cache_root = root_path.as_ref().to_path_buf();
    Self {
      cache_root: cache_root.clone(),

      tts_synthesizer_model_root: cache_root.join("tts/synthesizer_models/"),
      tts_pretrained_vocoder_model_root: cache_root.join("tts/vocoder_models_pretrained/"),
      vocoder_model_root: cache_root.join("tts/user_uploaded_vocoder_models/"),
      gpt_sovits_model_root: cache_root.join("tts/gpt_sovits_models/"),

      voice_conversion_model_root: cache_root.join("voice_conversion/models/"),

      w2l_model_root: cache_root.join("w2l/models/"),
      w2l_end_bump_root: cache_root.join("w2l/end_bumps/"),
      w2l_face_templates_root: cache_root.join("w2l/face_templates/"),
      w2l_templates_media_root: cache_root.join("w2l/template_media/"),
      video_asset_root: cache_root.join("static_video_assets/"),
    }
  }

  // ==================== TTS SYNTHESIZER MODELS ====================

  /// We cache TTS synthesizer models here.
  /// We'll likely need to LRU cache them.
  pub fn tts_synthesizer_model_path<P: AsRef<Path>>(&self, model_filename: P) -> PathBuf {
    self.tts_synthesizer_model_root.join(model_filename)
  }

  pub fn tts_synthesizer_model_directory(&self) -> &Path {
    &self.tts_synthesizer_model_root
  }

  pub fn create_tts_synthesizer_model_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.tts_synthesizer_model_directory())?;
    Ok(())
  }

  // ==================== TTS PRETRAINED VOCODER MODELS (WAVEGLOW, HIFIGAN, ETC) ====================

  /// We'll start with just a few vocoders, but this may grow.
  pub fn tts_pretrained_vocoder_model_path(&self, model_filename: &str) -> PathBuf {
    self.tts_pretrained_vocoder_model_root.join(model_filename)
  }

  pub fn tts_pretrained_vocoder_model_directory(&self) -> &Path {
    &self.tts_pretrained_vocoder_model_root
  }

  pub fn create_tts_pretrained_vocoder_model_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.tts_pretrained_vocoder_model_directory())?;
    Ok(())
  }

  // ==================== USER UPLOADED VOCODER MODELS ====================

  pub fn custom_vocoder_model_path(&self, model_filename: &str) -> PathBuf {
    self.vocoder_model_root.join(model_filename)
  }

  pub fn custom_vocoder_model_directory(&self) -> &Path {
    &self.vocoder_model_root
  }

  pub fn create_custom_vocoder_model_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.custom_vocoder_model_directory())?;
    Ok(())
  }

  // ==================== VOICE CONVERSION MODELS ====================

  /// We cache voice conversion models here.
  /// We'll likely need to LRU cache them.
  pub fn voice_conversion_model_path<P: AsRef<Path>>(&self, model_filename: P) -> PathBuf {
    self.voice_conversion_model_root.join(model_filename)
  }

  pub fn voice_conversion_model_directory(&self) -> &Path {
    &self.voice_conversion_model_root
  }

  pub fn create_voice_conversion_model_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.voice_conversion_model_directory())?;
    Ok(())
  }

  // ==================== W2L MODELS (there are only two of them) ====================

  /// There are only two pretrained W2L models, so we won't run out of space.
  pub fn w2l_model_path(&self, model_filename: &str) -> PathBuf {
    self.w2l_model_root.join(model_filename)
  }

  pub fn w2l_model_directory(&self) -> &Path {
    &self.w2l_model_root
  }

  pub fn create_w2l_model_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.w2l_model_directory())?;
    Ok(())
  }

  // ==================== W2L END BUMPS (short video logos) ====================

  pub fn w2l_end_bump_path(&self, end_bump_filename: &str) -> PathBuf {
    self.w2l_end_bump_root.join(end_bump_filename)
  }

  pub fn w2l_end_bump_directory(&self) -> &Path {
    &self.w2l_end_bump_root
  }

  pub fn create_w2l_end_bump_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.w2l_end_bump_directory())?;
    Ok(())
  }

  // ==================== W2L MEDIA ====================

  /// We cache W2L media here.
  /// We'll likely need to LRU cache them.
  pub fn w2l_template_media_path(&self, template_private_bucket_hash: &str) -> PathBuf {
    self.w2l_templates_media_root.join(template_private_bucket_hash)
  }

  pub fn w2l_template_media_directory(&self) -> &Path {
    &self.w2l_templates_media_root
  }

  pub fn create_w2l_template_media_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.w2l_template_media_directory())?;
    Ok(())
  }

  // ==================== W2L CACHED FACES ====================

  /// We cache W2L faces here.
  /// We'll likely need to LRU cache them.
  pub fn w2l_face_template_path(&self, template_private_bucket_hash: &str) -> PathBuf {
    let filename = format!("{}_detected_faces.pickle", template_private_bucket_hash);
    self.w2l_face_templates_root.join(filename)
  }

  pub fn w2l_face_template_directory(&self) -> &Path {
    &self.w2l_face_templates_root
  }

  pub fn create_w2l_face_template_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.w2l_face_template_directory())?;
    Ok(())
  }

  // ==================== VIDEO ASSETS (End bump, etc) ====================

  /// There is only a single end bump, and we'll add a watermark file.
  pub fn video_asset_path(&self) -> &Path {
    &self.video_asset_root
  }

  pub fn create_video_asset_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.video_asset_path())?;
    Ok(())
  }

  // ==================== GPT SOVITS MODELS ====================
  pub fn gpt_sovits_model_path(&self, model_filename: &str) -> PathBuf {
    self.gpt_sovits_model_root.join(model_filename)
  }

  pub fn gpt_sovits_model_directory(&self) -> &Path {
    &self.gpt_sovits_model_root
  }

  pub fn create_gpt_sovits_model_path(&self) -> AnyhowResult<()> {
    let _ = fs::create_dir_all(self.gpt_sovits_model_directory())?;
    Ok(())
  }

  // ==================== W2L OUTPUT RESULTS ====================

  // We cache W2L faces here.
  // We'll likely need to LRU cache them.
  //pub fn w2l_output_results_path(&self, temp_dir: &TempDir, inference_job_token: &str) -> PathBuf {
  //  // NB: We don't want colons from the token in the filename.
  //  let filename = inference_job_token.replace(":", "");
  //  let filename = format!("{}_result.mp4", filename);

  //  temp_dir.path().join(&filename)
  //}
}