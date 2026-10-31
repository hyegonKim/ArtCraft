
#[cfg(not(target_os = "macos"))]
pub use ml_models::ml::prompt_cache::PromptCache;

#[cfg(not(not(target_os = "macos")))]
pub struct PromptCache {
  // Intentionally left blank
}

#[cfg(not(not(target_os = "macos")))]
impl PromptCache {
  pub fn with_capacity(capacity: usize) -> anyhow::Result<Self> {
    Ok(Self {})
  }
}
