#[cfg(not(target_os = "macos"))]
pub use ml_models::ml::model_config::ModelConfig;

#[cfg(not(not(target_os = "macos")))]
pub struct ModelConfig {
  // Intentionally left blank
}

#[cfg(not(not(target_os = "macos")))]
impl ModelConfig {
  pub fn init() -> anyhow::Result<Self> {
    Ok(Self {})
  }
}
