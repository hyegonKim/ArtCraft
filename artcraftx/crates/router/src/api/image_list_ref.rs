use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::image_ref::ImageRef;

#[derive(Clone, Debug)]
pub enum ImageListRef {
  MediaFileTokens(Vec<MediaFileToken>),
  Urls(Vec<String>),
  /// Mixed per-item sources (tokens, URLs, local paths, bytes) — the general
  /// form; the homogeneous variants above remain as conveniences.
  Sources(Vec<ImageRef>),
}

impl ImageListRef {
  /// Normalize any variant into per-item refs, in order.
  pub fn into_refs(self) -> Vec<ImageRef> {
    match self {
      Self::MediaFileTokens(tokens) => tokens.into_iter().map(ImageRef::MediaFileToken).collect(),
      Self::Urls(urls) => urls.into_iter().map(ImageRef::Url).collect(),
      Self::Sources(refs) => refs,
    }
  }

  pub fn len(&self) -> usize {
    match self {
      Self::MediaFileTokens(tokens) => tokens.len(),
      Self::Urls(urls) => urls.len(),
      Self::Sources(refs) => refs.len(),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}
