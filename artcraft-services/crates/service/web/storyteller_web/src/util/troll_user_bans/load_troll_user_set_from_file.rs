use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use errors::AnyhowResult;

use crate::util::troll_user_bans::troll_user_set::TrollUserSet;

pub fn load_user_token_set_from_file<P: AsRef<Path>>(path: P) -> AnyhowResult<TrollUserSet> {
  let file = File::open(path)?;
  let reader = BufReader::new(file);

  let lines = reader.lines()
      .filter_map(|line| line.ok())
      .map(|line| line.trim().to_string())
      .filter(|line| !(line.starts_with("#") || line.is_empty()))
      .collect::<HashSet<String>>();

  Ok(TrollUserSet::from_set(lines))
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use tokens::tokens::users::UserToken;

  use crate::util::troll_user_bans::load_troll_user_set_from_file::load_user_token_set_from_file;

  fn test_file(path_from_repo_root: &str) -> PathBuf {
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("../../../../{}", path_from_repo_root));
    path
  }

  #[test]
  fn test_load_user_token_set_from_file() {
    let filename = test_file("test_data/text_files/troll_user_token_ban_example/user_tokens_and_comments_1.txt");
    let user_token_set = load_user_token_set_from_file(filename).unwrap();

    // Comments are not included
    assert_eq!(user_token_set.contains_user_token("# this is test data"), false);

    // User tokens in the file are.
    assert_eq!(user_token_set.contains_user_token("U:FOO"), true);
    assert_eq!(user_token_set.contains_user_token("U:BAR"), true);
    assert_eq!(user_token_set.contains_user_token("U:NINTENDO"), false);

    // Typed interface
    assert_eq!(user_token_set.contains_user_token_typed(&UserToken::new_from_str("U:FOO")), true);
    assert_eq!(user_token_set.contains_user_token_typed(&UserToken::new_from_str("U:NINTENDO")), false);

    // Length is expected
    assert_eq!(user_token_set.len(), 2);
  }
}
