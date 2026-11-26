//! Scoped temporary folder helper for UI tests.

use std::ops::Deref;
use std::path::{Path, PathBuf};

pub(crate) struct Folder(PathBuf);

impl Folder {
    pub(crate) fn new(tag: &str) -> Self {
        Self::adopt(vectorcraft_testkit::temp_dir(tag))
    }

    pub(crate) fn adopt(path: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn file(&self, name: &str, bytes: &[u8]) -> String {
        let p = self.0.join(name);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&p, bytes).unwrap();
        p.to_string_lossy().into_owned()
    }
}

impl Deref for Folder {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
