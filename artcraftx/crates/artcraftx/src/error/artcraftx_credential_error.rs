use core_types::enums::generation_source::{CredentialKind, GenerationSource};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

/// Errors from loading and saving credential TOML files.
#[derive(Debug)]
pub enum ArtcraftXCredentialError {
  // Filesystem errors
  DirectoryReadError { path: PathBuf, source: std::io::Error },
  FileReadError { path: PathBuf, source: std::io::Error },
  FileWriteError { path: PathBuf, source: std::io::Error },
  FileDeleteError { path: PathBuf, source: std::io::Error },
  // File name errors (ids are file names within the credentials directory)
  InvalidFileName { file_name: String },
  // Serialization errors
  TomlParseError { path: PathBuf, source: toml::de::Error },
  TomlSerializeError { source: toml::ser::Error },
  // Validation errors
  MissingSecret { path: PathBuf },
  AmbiguousSecret { path: PathBuf },
  SecretKindMismatch { path: PathBuf, service: GenerationSource, found: CredentialKind },
}

impl Error for ArtcraftXCredentialError {}

impl Display for ArtcraftXCredentialError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::DirectoryReadError { path, source } => {
        write!(f, "DirectoryReadError: {:?}: {:?}", path, source)
      },
      Self::FileReadError { path, source } => {
        write!(f, "FileReadError: {:?}: {:?}", path, source)
      },
      Self::FileWriteError { path, source } => {
        write!(f, "FileWriteError: {:?}: {:?}", path, source)
      },
      Self::FileDeleteError { path, source } => {
        write!(f, "FileDeleteError: {:?}: {:?}", path, source)
      },
      Self::InvalidFileName { file_name } => {
        write!(f, "InvalidFileName: {:?}", file_name)
      },
      Self::TomlParseError { path, source } => {
        write!(f, "TomlParseError: {:?}: {:?}", path, source)
      },
      Self::TomlSerializeError { source } => {
        write!(f, "TomlSerializeError: {:?}", source)
      },
      Self::MissingSecret { path } => {
        write!(f, "MissingSecret (neither cookie nor api_key present): {:?}", path)
      },
      Self::AmbiguousSecret { path } => {
        write!(f, "AmbiguousSecret (both cookie and api_key present): {:?}", path)
      },
      Self::SecretKindMismatch { path, service, found } => {
        write!(
          f,
          "SecretKindMismatch (service {:?} expects {:?}, found {:?}): {:?}",
          service,
          service.kind(),
          found,
          path,
        )
      },
    }
  }
}
