//! Fixed desktop-app registry and executable discovery. No command string or shell is used.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const SUITE_APPS: [SuiteApp; 13] = [
  SuiteApp { id: "photocraft", name: "PhotoCraft", domain: "Photo editing" },
  SuiteApp { id: "vectorcraft", name: "VectorCraft", domain: "Vector illustration" },
  SuiteApp { id: "filmcraft", name: "FilmCraft", domain: "Video editing" },
  SuiteApp { id: "lightcraft", name: "LightCraft", domain: "Photo development" },
  SuiteApp { id: "effectcraft", name: "EffectCraft", domain: "Motion graphics and effects" },
  SuiteApp { id: "designcraft", name: "DesignCraft", domain: "Page layout" },
  SuiteApp { id: "soundcraft", name: "SoundCraft", domain: "Audio editing" },
  SuiteApp { id: "figcraft", name: "FigCraft", domain: "Interface design" },
  SuiteApp { id: "wordcraft", name: "WordCraft", domain: "Word processing" },
  SuiteApp { id: "cadcraft", name: "CadCraft", domain: "CAD drafting" },
  SuiteApp { id: "deckcraft", name: "DeckCraft", domain: "Presentations" },
  SuiteApp { id: "gridcraft", name: "GridCraft", domain: "Spreadsheets" },
  SuiteApp { id: "artcraft-launcher", name: "Craft Launcher", domain: "App management" },
];

#[derive(Clone, Copy, Debug)]
pub struct SuiteApp {
  pub id: &'static str,
  pub name: &'static str,
  pub domain: &'static str,
}

/// An explicit directory takes precedence; an invalid override never silently falls back.
pub fn resolve_binary(id: &str, override_dir: Option<&Path>, current_exe: &Path) -> Result<PathBuf, String> {
  let app = SUITE_APPS.iter().find(|app| app.id == id).ok_or("Unknown Craft Suite app")?;
  let directory = override_dir.or_else(|| current_exe.parent()).ok_or("Couldn't locate the app folder")?;
  if !directory.is_absolute() || !directory.is_dir() {
    return Err("The Craft Suite app folder is unavailable".into());
  }
  let binary = directory.join(format!("{}{}", app.id, std::env::consts::EXE_SUFFIX));
  let metadata = binary.metadata().map_err(|_| format!("{} is not installed here", app.name))?;
  if !metadata.is_file() {
    return Err(format!("{} is not an application file", app.name));
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o111 == 0 {
      return Err(format!("{} cannot be opened: executable permission is missing", app.name));
    }
  }
  Ok(binary)
}

/// Re-resolve immediately before launching. Only a registry app can become a process.
pub fn launch(id: &str, override_dir: Option<&Path>, current_exe: &Path) -> Result<(), String> {
  let binary = resolve_binary(id, override_dir, current_exe)?;
  let mut child = Command::new(&binary).spawn().map_err(|err| format!("Couldn't open the app: {err}"))?;
  // Reap when the separate app exits without blocking ArtCraft's UI thread.
  std::thread::spawn(move || {
    let _ = child.wait();
  });
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use std::sync::atomic::{AtomicU64, Ordering};

  static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

  #[test]
  fn registry_contains_unique_desktop_binary_names() {
    let mut names = std::collections::HashSet::new();
    for app in SUITE_APPS {
      assert!(names.insert(app.id));
      assert!(app.id.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'));
    }
    assert_eq!(SUITE_APPS.len(), 13);
  }

  #[test]
  fn finds_sibling_apps_and_prefers_an_explicit_directory() {
    let sibling = TestDir::new();
    let other = TestDir::new();
    let sibling_binary = sibling.app("photocraft");
    let other_binary = other.app("photocraft");
    let current_exe = sibling.0.join("artcraft-native");
    assert_eq!(resolve_binary("photocraft", None, &current_exe).unwrap(), sibling_binary);
    assert_eq!(resolve_binary("photocraft", Some(&other.0), &current_exe).unwrap(), other_binary);
  }

  #[test]
  fn rejects_unknown_names_and_path_traversal() {
    let directory = TestDir::new();
    for id in ["../photocraft", "/bin/sh", "photocraft;echo injected", "unknown"] {
      assert!(resolve_binary(id, None, &directory.0.join("artcraft-native")).is_err());
    }
  }

  #[test]
  fn missing_override_does_not_fall_back_to_an_existing_sibling() {
    let directory = TestDir::new();
    directory.app("photocraft");
    let current_exe = directory.0.join("artcraft-native");
    assert!(resolve_binary("photocraft", Some(&directory.0.join("missing")), &current_exe).is_err());
    assert!(resolve_binary("photocraft", Some(Path::new("relative")), &current_exe).is_err());
  }

  #[test]
  fn missing_apps_and_directories_are_not_launchable() {
    let directory = TestDir::new();
    let current_exe = directory.0.join("artcraft-native");
    assert!(resolve_binary("photocraft", None, &current_exe).is_err());
    assert!(launch("photocraft", None, &current_exe).is_err());
    fs::create_dir(directory.0.join(format!("photocraft{}", std::env::consts::EXE_SUFFIX))).unwrap();
    assert!(resolve_binary("photocraft", None, &current_exe).is_err());
  }

  #[cfg(unix)]
  #[test]
  fn a_nonexecutable_file_is_not_launchable() {
    use std::os::unix::fs::PermissionsExt;
    let directory = TestDir::new();
    let binary = directory.app("photocraft");
    fs::set_permissions(binary, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(resolve_binary("photocraft", None, &directory.0.join("artcraft-native")).is_err());
  }

  struct TestDir(PathBuf);

  impl TestDir {
    fn new() -> Self {
      let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
      let path = std::env::temp_dir().join(format!("artcraft-suite-test-{}-{nonce}-{}", std::process::id(), NEXT_DIR.fetch_add(1, Ordering::Relaxed)));
      fs::create_dir(&path).unwrap();
      Self(path)
    }

    fn app(&self, id: &str) -> PathBuf {
      let path = self.0.join(format!("{id}{}", std::env::consts::EXE_SUFFIX));
      fs::write(&path, b"test fixture; never executed").unwrap();
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
      }
      path
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }
}
