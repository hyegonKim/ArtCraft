use std::path::PathBuf;

pub trait Document {
  /// Get the document unique identifier
  fn get_document_id(&self) -> String;

  /// Return the document path
  fn get_document_path(&self) -> PathBuf;
}
