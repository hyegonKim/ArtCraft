use std::collections::HashSet;

use tokens::tokens::users::UserToken;

#[derive(Clone)]
pub struct TrollUserSet {
  pub user_token_set: HashSet<String>,
}

impl TrollUserSet {

  pub fn new() -> Self {
    Self {
      user_token_set: HashSet::new(),
    }
  }

  pub fn from_set(user_token_set: HashSet<String>) -> Self {
    Self {
      user_token_set,
    }
  }

  pub fn replace_set(&mut self, user_token_set: HashSet<String>) {
    self.user_token_set = user_token_set;
  }

  pub fn add_user_token(&mut self, user_token: String) -> bool {
    self.user_token_set.insert(user_token)
  }

  pub fn add_user_token_typed(&mut self, user_token: UserToken) -> bool {
    self.user_token_set.insert(user_token.to_string())
  }

  pub fn remove_user_token(&mut self, user_token: &str) -> bool {
    self.user_token_set.remove(user_token)
  }

  pub fn contains_user_token<S: AsRef<str>>(&self, user_token: S) -> bool {
    self.user_token_set.contains(user_token.as_ref())
  }

  pub fn contains_user_token_typed(&self, user_token: &UserToken) -> bool {
    self.user_token_set.contains(user_token.as_str())
  }

  pub fn len(&self) -> usize {
    self.user_token_set.len()
  }

  pub fn is_empty(&self) -> bool {
    self.user_token_set.is_empty()
  }
}
