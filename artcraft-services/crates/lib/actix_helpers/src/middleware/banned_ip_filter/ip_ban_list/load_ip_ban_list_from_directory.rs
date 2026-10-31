use std::path::Path;

use errors::AnyhowResult;

use crate::middleware::banned_ip_filter::ip_ban_list::ip_ban_list::IpBanList;
use crate::middleware::banned_ip_filter::ip_ban_list::load_ip_set_from_file::load_ip_set_from_file;

pub fn load_ip_ban_list_from_directory<P: AsRef<Path>>(path: P) -> AnyhowResult<IpBanList> {
  let ip_ban_list = IpBanList::new();
  let paths = std::fs::read_dir(path)?;

  for entry in paths {
    let path = entry?.path();
    if ignore_path(&path) {
      continue;
    }
    let path_name = path.to_string_lossy().to_string();
    let ip_set = load_ip_set_from_file(path)?;
    if ip_set.is_empty() {
      continue;
    }
    ip_ban_list.add_set(path_name, ip_set)?;
  }

  Ok(ip_ban_list)
}

fn ignore_path(path: &Path) -> bool {
  // NB: Path is quoted for some reason and fails ends_with() etc., so we convert it to a string.
  let test_path = path.to_string_lossy();
  test_path.ends_with("~")
}

#[cfg(test)]
mod tests {
  use std::path::{Path, PathBuf};

  use crate::middleware::banned_ip_filter::ip_ban_list::load_ip_ban_list_from_directory::{ignore_path, load_ip_ban_list_from_directory};

  fn test_file(path_from_repo_root: &str) -> PathBuf {
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("../../../{}", path_from_repo_root));
    path
  }

  #[test]
  fn test_ignore_paths() {
    // Good
    assert_eq!(false, ignore_path(Path::new("file.txt")));
    assert_eq!(false, ignore_path(Path::new("file")));

    // Vim files, private files, etc.
    assert_eq!(true, ignore_path(Path::new("file.txt~")));
  }

  #[test]
  fn test_load_ip_ban_list_from_directory() {
    let directory = test_file("test_data/text_files/ip_ban_example/");
    let ip_set = load_ip_ban_list_from_directory(directory).unwrap();

    // Comments are not included
    assert_eq!(ip_set.contains_ip_address("# this is test data").unwrap(), false);

    // IP addresses in both files are
    assert_eq!(ip_set.contains_ip_address("127.0.0.1").unwrap(), true);
    assert_eq!(ip_set.contains_ip_address("192.168.0.1").unwrap(), true);

    // All five IPs were loaded
    assert_eq!(ip_set.total_ip_address_count().unwrap(), 5);
  }

  #[test]
  fn empty_files_are_not_loaded() {
    let directory = test_file("test_data/text_files/ip_ban_example/");
    let ip_set = load_ip_ban_list_from_directory(directory).unwrap();

    // NB: There are two files with IP addresses and a single empty file, which should not be loaded
    assert_eq!(ip_set.set_count().unwrap(), 2);
  }
}
