//! Explicit isolated launches never consult the normal account, settings, or UI storage.

use std::ffi::OsString;
use std::path::{Component, PathBuf};

use artcraft_client::utils::api_host::ApiHost;

const USAGE: &str = "Usage: artcraft-native [--data-dir <absolute-directory> --api-base-url http://localhost:<port>]";

#[derive(Clone, Debug, Default)]
pub enum LaunchOptions {
  #[default]
  Default,
  Isolated { data_dir: PathBuf, api_port: u32 },
}

pub struct RuntimeSettings {
  pub data_root: PathBuf,
  pub api_host: ApiHost,
  pub import_tauri_cookies: bool,
}

impl LaunchOptions {
  /// Validate every argument before resolving any filesystem or account defaults.
  pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
    let mut args = args.into_iter();
    let (mut data_dir, mut api_port) = (None, None);
    while let Some(arg) = args.next() {
      match arg.to_str() {
        Some("--data-dir") => {
          if data_dir.is_some() {
            return Err(format!("--data-dir must be specified once. {USAGE}"));
          }
          let path = PathBuf::from(args.next().ok_or_else(|| format!("Missing --data-dir value. {USAGE}"))?);
          if !path.is_absolute() || path.components().any(|part| matches!(part, Component::ParentDir)) {
            return Err("--data-dir must be an absolute path without parent components".into());
          }
          data_dir = Some(path);
        },
        Some("--api-base-url") => {
          if api_port.is_some() {
            return Err(format!("--api-base-url must be specified once. {USAGE}"));
          }
          let value = args.next().ok_or_else(|| format!("Missing --api-base-url value. {USAGE}"))?;
          let url = value.to_str().and_then(|value| url::Url::parse(value).ok())
            .ok_or_else(|| "Invalid --api-base-url".to_owned())?;
          // The shared ApiHost represents local endpoints by port on localhost.
          // Reject unsupported URL components instead of silently changing their meaning.
          if url.scheme() != "http" || url.host_str() != Some("localhost") || !url.username().is_empty()
            || url.password().is_some() || url.path() != "/" || url.query().is_some() || url.fragment().is_some()
          {
            return Err("--api-base-url must be http://localhost[:port] without credentials, path, query, or fragment".into());
          }
          let port = url.port_or_known_default().ok_or_else(|| "Missing API port".to_owned())?;
          if port == 0 {
            return Err("--api-base-url port must be nonzero".into());
          }
          api_port = Some(u32::from(port));
        },
        _ => return Err(format!("Unrecognized option. {USAGE}")),
      }
    }
    match (data_dir, api_port) {
      (None, None) => Ok(Self::Default),
      (Some(data_dir), Some(api_port)) => Ok(Self::Isolated { data_dir, api_port }),
      _ => Err(format!("--data-dir and --api-base-url must be supplied together. {USAGE}")),
    }
  }

  pub fn resolve(&self, defaults: impl FnOnce() -> (PathBuf, ApiHost)) -> RuntimeSettings {
    match self {
      Self::Default => {
        let (data_root, api_host) = defaults();
        RuntimeSettings { data_root, api_host, import_tauri_cookies: true }
      },
      Self::Isolated { data_dir, api_port } => RuntimeSettings {
        data_root: data_dir.clone(), api_host: ApiHost::Localhost { port: *api_port }, import_tauri_cookies: false,
      },
    }
  }

  pub fn native_options(&self, viewport: egui::ViewportBuilder) -> eframe::NativeOptions {
    let persistence_path = match self {
      Self::Default => None,
      Self::Isolated { data_dir, .. } => Some(data_dir.join("ui").join("app.ron")),
    };
    eframe::NativeOptions { viewport, persist_window: true, persistence_path, ..Default::default() }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_launch_keeps_existing_defaults_and_storage() {
    let launch = parse(&[]).unwrap();
    let settings = launch.resolve(|| (PathBuf::from("/test-default-root"), ApiHost::Storyteller));
    assert_eq!(settings.data_root, PathBuf::from("/test-default-root"));
    assert!(settings.import_tauri_cookies);
    assert!(matches!(settings.api_host, ApiHost::Storyteller));
    assert!(launch.native_options(egui::ViewportBuilder::default()).persistence_path.is_none());
  }

  #[test]
  fn isolated_launch_never_resolves_global_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("isolated");
    let launch = LaunchOptions::parse([
      OsString::from("--api-base-url"), OsString::from("http://localhost:4203"),
      OsString::from("--data-dir"), root.clone().into_os_string(),
    ]).unwrap();
    let settings = launch.resolve(|| panic!("isolated launch must not read global settings"));
    assert_eq!(settings.data_root, root);
    assert!(!settings.import_tauri_cookies);
    assert_eq!(settings.api_host.to_api_hostname_and_scheme(), "http://localhost:4203");
    let native = launch.native_options(egui::ViewportBuilder::default());
    assert_eq!(native.persistence_path, Some(root.join("ui/app.ron")));
    assert!(native.persist_window);
    assert!(!root.exists(), "argument parsing and option resolution must not create directories");
  }

  #[test]
  fn malformed_arguments_fail_before_filesystem_io() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("must-not-exist");
    let result = LaunchOptions::parse([
      OsString::from("--data-dir"), root.clone().into_os_string(),
      OsString::from("--api-base-url"), OsString::from("https://production.invalid"),
    ]);
    assert!(result.is_err());
    assert!(!root.exists());
    for args in [
      vec!["--unknown"], vec!["unexpected"], vec!["--data-dir"], vec!["--api-base-url"],
      vec!["--data-dir", "/tmp/isolated"], vec!["--api-base-url", "http://localhost:4203"],
      vec!["--data-dir", "relative", "--api-base-url", "http://localhost:4203"],
      vec!["--data-dir", "/tmp/../outside", "--api-base-url", "http://localhost:4203"],
      vec!["--data-dir", "/tmp/a", "--data-dir", "/tmp/b", "--api-base-url", "http://localhost:4203"],
      vec!["--data-dir", "/tmp/a", "--api-base-url", "http://localhost:4203", "--api-base-url", "http://localhost:4204"],
    ] {
      assert!(parse(&args).is_err(), "invalid invocation must fail: {args:?}");
    }
  }

  #[test]
  fn unsupported_api_urls_are_rejected_instead_of_normalized() {
    for value in [
      "not a URL", "https://localhost:4203", "http://production.invalid", "http://127.0.0.1:4203",
      "http://localhost:0", "http://localhost:70000", "http://user:pass@localhost:4203",
      "http://localhost:4203/api", "http://localhost:4203/?route=production", "http://localhost:4203/#fragment",
    ] {
      assert!(parse(&["--data-dir", "/tmp/isolated", "--api-base-url", value]).is_err(), "unsupported endpoint must fail");
    }
  }

  fn parse(args: &[&str]) -> Result<LaunchOptions, String> {
    LaunchOptions::parse(args.iter().map(|value| OsString::from(*value)))
  }
}
