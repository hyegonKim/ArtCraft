use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize env_logger so live-API tests print request/response details.
/// Idempotent — safe to call from every test.
pub fn setup_test_logging() {
  INIT.call_once(|| {
    let _ = env_logger::builder()
      .filter_level(log::LevelFilter::Debug)
      .is_test(true)
      .try_init();
  });
}
