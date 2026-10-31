use errors::AnyhowResult;

pub trait JobProgressReporterBuilder {
  // NB: Still learning "Box<dyn Trait>" vs "impl Trait";
  // The former is dynamic dispatch, dynamic size, vtable-dispatched. The latter is static dispatch,
  // Sized, and known at compile time:
  // https://users.rust-lang.org/t/difference-between-returning-dyn-box-trait-and-impl-trait/57640/3

  fn new_generic_download(&self, job_token: &str) -> AnyhowResult<Box<dyn JobProgressReporter>>;

  fn new_generic_inference(&self, job_token: &str) -> AnyhowResult<Box<dyn JobProgressReporter>>;

  fn new_tts_download(&self, tts_job_token: &str) -> AnyhowResult<Box<dyn JobProgressReporter>>;

  fn new_tts_inference(&self, tts_job_token: &str) -> AnyhowResult<Box<dyn JobProgressReporter>>;

  fn new_w2l_download(&self, w2l_job_token: &str) -> AnyhowResult<Box<dyn JobProgressReporter>>;

  fn new_w2l_inference(&self, w2l_job_token: &str) -> AnyhowResult<Box<dyn JobProgressReporter>>;
}

pub trait JobProgressReporter {
  /// Record the job progress "perhaps somewhere" so the frontend can see what's going on.
  /// Occasionally you may want to replace this with a no-op.
  fn log_status(&mut self, logging_details: &str) -> AnyhowResult<()>;
}