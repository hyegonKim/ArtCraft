use std::fmt::{Debug, Formatter};

/// In-memory media for a reference the caller already holds as bytes (e.g. a
/// pasted image that never touched disk). The file name, when known, helps
/// guess the MIME type; the bytes themselves are still sniffed first.
#[derive(Clone)]
pub struct MediaBytes {
  pub bytes: Vec<u8>,
  pub maybe_file_name: Option<String>,
}

impl MediaBytes {
  pub fn new(bytes: Vec<u8>) -> Self {
    Self { bytes, maybe_file_name: None }
  }

  pub fn with_file_name(mut self, file_name: impl Into<String>) -> Self {
    self.maybe_file_name = Some(file_name.into());
    self
  }
}

// Manual impl: requests carrying these are logged; never dump the bytes.
impl Debug for MediaBytes {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("MediaBytes")
        .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
        .field("maybe_file_name", &self.maybe_file_name)
        .finish()
  }
}
