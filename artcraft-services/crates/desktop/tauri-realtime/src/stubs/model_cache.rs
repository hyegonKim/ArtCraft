#[cfg(not(target_os = "macos"))]
pub use ml_models::ml::model_cache::ModelCache;

#[cfg(not(not(target_os = "macos")))]
pub struct ModelCache {
  // Intentionally left blank
}

#[cfg(not(not(target_os = "macos")))]
impl ModelCache {
  pub fn new() -> Self {
    Self {}
  }
}
