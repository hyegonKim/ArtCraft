use std::path::Path;

use errors::AnyhowResult;

use crate::util::troll_user_bans::load_troll_user_set_from_file::load_user_token_set_from_file;
use crate::util::troll_user_bans::troll_user_ban_list::TrollUserBanList;

pub fn load_user_token_ban_list_from_directory<P: AsRef<Path>>(path: P) -> AnyhowResult<TrollUserBanList> {
  let user_token_ban_list = TrollUserBanList::new();
  let paths = std::fs::read_dir(path)?;

  for entry in paths {
    let path = entry?.path();
    if ignore_path(&path) {
      continue;
    }
    let path_name = path.to_string_lossy().to_string();
    let user_token_set = load_user_token_set_from_file(path)?;
    if user_token_set.is_empty() {
      continue;
    }
    user_token_ban_list.add_set(path_name, user_token_set)?;
  }

  Ok(user_token_ban_list)
}

fn ignore_path(path: &Path) -> bool {
  // NB: Path is quoted for some reason and fails ends_with() etc., so we convert it to a string.
  let test_path = path.to_string_lossy();
  test_path.ends_with("~")
}

#[cfg(test)]
mod tests {
  use std::path::{Path, PathBuf};

  use crate::util::troll_user_bans::load_troll_user_ban_list_from_directory::{ignore_path, load_user_token_ban_list_from_directory};

  fn test_file(path_from_repo_root: &str) -> PathBuf {
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("../../../../{}", path_from_repo_root));
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
  fn test_load_user_token_ban_list_from_directory() {
    let directory = test_file("test_data/text_files/troll_user_token_ban_example/");
    let user_token_set = load_user_token_ban_list_from_directory(directory).unwrap();

    // Comments are not included
    assert_eq!(user_token_set.contains_user_token("# this is test data").unwrap(), false);

    // User tokens in both files are
    assert_eq!(user_token_set.contains_user_token("U:FOO").unwrap(), true);
    assert_eq!(user_token_set.contains_user_token("U:BOB").unwrap(), true);

    // All five user tokens were loaded
    assert_eq!(user_token_set.total_user_token_count().unwrap(), 5);
  }

  #[test]
  fn empty_files_are_not_loaded() {
    let directory = test_file("test_data/text_files/troll_user_token_ban_example/");
    let user_token_set = load_user_token_ban_list_from_directory(directory).unwrap();

    // NB: There are two files with user tokens and a single empty file, which should not be loaded
    assert_eq!(user_token_set.set_count().unwrap(), 2);
  }
}
