use std::env;
use std::io;
use std::path::PathBuf;

/// The "expanduser" crate doesn't compile on Windows, so we replace its functionality slightly.
/// Supports both `~/...` and `~\...` across platforms.
pub fn expanduser<P: AsRef<str>>(path: P) -> io::Result<PathBuf> {
  let s = path.as_ref();
  if s == "~" {
    return home_dir();
  }
  if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
    let home = home_dir()?;
    return Ok(home.join(rest));
  }
  Ok(PathBuf::from(s))
}

pub fn home_dir() -> io::Result<PathBuf> {
  env::var_os("HOME")
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
    .or_else(dirs::home_dir)
    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no home directory is set"))
}

#[cfg(test)]
mod tests {
  use crate::core::state::expanduser::expanduser;
  use std::env;
  use std::path::PathBuf;

  #[test]
  #[serial_test::serial]
  fn test_success() {
    let old_home = env::var_os("HOME");
    let new_home = "/home/foo";
    env::set_var("HOME", new_home);
    let path = expanduser("~/path/to/directory");
    match old_home {
      Some(v) => env::set_var("HOME", v),
      None => env::remove_var("HOME"),
    }
    assert_eq!(path.expect("io error"), PathBuf::from("/home/foo").join("path/to/directory"));
  }

  #[test]
  #[serial_test::serial]
  fn test_only_tilde() {
    let old_home = env::var_os("HOME");
    let new_home = "/home/foo";
    env::set_var("HOME", new_home);
    let pathstr = "~";
    let path = expanduser(pathstr);
    match old_home {
      Some(v) => env::set_var("HOME", v),
      None => env::remove_var("HOME"),
    }
    assert_eq!(path.expect("io error"), PathBuf::from("/home/foo"));
  }
}
