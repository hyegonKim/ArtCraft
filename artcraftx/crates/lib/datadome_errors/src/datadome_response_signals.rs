/// Everything about a response that helps decide whether DataDome produced
/// it. Build with [`Self::new`] and add whichever headers the HTTP client
/// exposes; the body-only path works without any.
#[derive(Debug, Clone, Default)]
pub struct DataDomeResponseSignals<'a> {
  pub status_code: u16,

  pub body: &'a str,

  /// The `x-datadome` header (`protected` on responses DataDome inspected).
  pub maybe_x_datadome: Option<&'a str>,

  /// The `x-dd-b` header: present on responses DataDome blocked.
  pub maybe_x_dd_b: Option<&'a str>,
}

impl<'a> DataDomeResponseSignals<'a> {
  pub fn new(status_code: u16, body: &'a str) -> Self {
    Self {
      status_code,
      body,
      maybe_x_datadome: None,
      maybe_x_dd_b: None,
    }
  }

  pub fn with_x_datadome(mut self, value: Option<&'a str>) -> Self {
    self.maybe_x_datadome = value;
    self
  }

  pub fn with_x_dd_b(mut self, value: Option<&'a str>) -> Self {
    self.maybe_x_dd_b = value;
    self
  }
}
