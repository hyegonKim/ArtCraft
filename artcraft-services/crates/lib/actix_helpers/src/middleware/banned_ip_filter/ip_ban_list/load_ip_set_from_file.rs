use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use errors::AnyhowResult;

use crate::middleware::banned_ip_filter::ip_ban_list::ip_set::IpSet;

pub fn load_ip_set_from_file<P: AsRef<Path>>(path: P) -> AnyhowResult<IpSet> {
  let file = File::open(path)?;
  let reader = BufReader::new(file);

  let lines = reader.lines()
      .filter_map(|line| line.ok())
      .map(|line| line.trim().to_string())
      .filter(|line| !(line.starts_with("#") || line.is_empty()))
      .collect::<HashSet<String>>();

  Ok(IpSet::from_set(lines))
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use crate::middleware::banned_ip_filter::ip_ban_list::load_ip_set_from_file::load_ip_set_from_file;

  fn test_file(path_from_repo_root: &str) -> PathBuf {
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("../../../{}", path_from_repo_root));
    path
  }

  #[test]
  fn test_load_ip_set_from_file() {
    let filename = test_file("test_data/text_files/ip_ban_example/local_ip_addresses_and_comments.txt");
    let ip_set = load_ip_set_from_file(filename).unwrap();

    // Comments are not included
    assert_eq!(ip_set.contains_ip_address("# this is test data"), false);

    // IP addresses in the file are.
    assert_eq!(ip_set.contains_ip_address("127.0.0.1"), true);
    assert_eq!(ip_set.contains_ip_address("192.168.1.1"), false);

    // Length is expected
    assert_eq!(ip_set.len(), 2);
  }
}
