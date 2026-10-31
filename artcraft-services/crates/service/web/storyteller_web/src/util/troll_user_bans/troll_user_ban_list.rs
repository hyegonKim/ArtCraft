use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use errors::{anyhow, AnyhowResult};

use crate::util::troll_user_bans::troll_user_set::TrollUserSet;

#[derive(Clone)]
pub struct TrollUserBanList {
  user_token_sets: Arc<RwLock<HashMap<String, TrollUserSet>>>
}

impl TrollUserBanList {
  pub fn new() -> Self {
    Self {
      user_token_sets: Arc::new(RwLock::new(HashMap::new()))
    }
  }

  pub fn contains_user_token<S: AsRef<str>>(&self, user_token: S) -> AnyhowResult<bool> {
    match self.user_token_sets.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(sets) => {
        for set in sets.values() {
          if set.contains_user_token(user_token.as_ref()) {
            return Ok(true)
          }
        }
        Ok(false)
      },
    }
  }

  pub fn add_set(&self, set_name: String, user_token_set: TrollUserSet) -> AnyhowResult<Option<TrollUserSet>> {
    match self.user_token_sets.write() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(mut sets) => {
        Ok(sets.insert(set_name, user_token_set))
      },
    }
  }

  pub fn remove_set(&self, set_name: &str) -> AnyhowResult<Option<TrollUserSet>> {
    match self.user_token_sets.write() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(mut sets) => {
        Ok(sets.remove(set_name))
      },
    }
  }

  pub fn total_user_token_count(&self) -> AnyhowResult<usize> {
    match self.user_token_sets.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(sets) => {
        Ok(sets.values()
            .map(|set| set.len())
            .sum())
      },
    }
  }

  pub fn set_count(&self) -> AnyhowResult<usize> {
    match self.user_token_sets.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(sets) => {
        Ok(sets.len())
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::util::troll_user_bans::troll_user_ban_list::TrollUserBanList;
  use crate::util::troll_user_bans::troll_user_set::TrollUserSet;

  #[test]
  fn test_contains_user_token() {
    let mut user_token_set_1 = TrollUserSet::new();
    user_token_set_1.add_user_token("U:FOO".to_string());
    user_token_set_1.add_user_token("U:BAR".to_string());

    let mut user_token_set_2 = TrollUserSet::new();
    user_token_set_2.add_user_token("U:JIM".to_string());
    user_token_set_2.add_user_token("U:CRAMER".to_string());

    let user_token_ban_list = TrollUserBanList::new();
    user_token_ban_list.add_set("set_1".to_string(), user_token_set_1).unwrap();
    user_token_ban_list.add_set("set_2".to_string(), user_token_set_2).unwrap();

    assert_eq!(user_token_ban_list.contains_user_token("U:FOO").unwrap(), true);
    assert_eq!(user_token_ban_list.contains_user_token("U:JIM").unwrap(), true);

    assert_eq!(user_token_ban_list.contains_user_token("U:BANKSY").unwrap(), false);
  }
}
