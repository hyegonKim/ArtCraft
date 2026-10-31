//! This is meant to disambiguate whether the deletion should occur as a user or moderator
//! for soft-deleted records. All such requests have a similar shape, and this was tested
//! with a truth table.

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DeleteRole {
  ErrorDoNotDelete,
  AsUser,
  AsMod,
}

pub fn delete_role_disambiguation(
  user_is_mod: bool,
  user_is_author: bool,
  as_mod_flag: Option<bool>
) -> DeleteRole {
  if !user_is_mod && !user_is_author {
    return DeleteRole::ErrorDoNotDelete;
  }
  // NB: Explored this with a truth table.
  let as_mod_flag_value = as_mod_flag.unwrap_or(false);
  let delete_as_mod = user_is_mod && !(user_is_author && as_mod_flag.is_some() && !as_mod_flag_value);
  if delete_as_mod { DeleteRole::AsMod } else { DeleteRole::AsUser }
}

#[cfg(test)]
mod tests {
  use crate::util::delete_role_disambiguation::{delete_role_disambiguation, DeleteRole};

  #[test]
  fn test_delete_as_mod() {
    // User is neither author nor mod.
    // Do not delete!
    assert_eq!(DeleteRole::ErrorDoNotDelete, delete_role_disambiguation(false, false, None));
    assert_eq!(DeleteRole::ErrorDoNotDelete, delete_role_disambiguation(false, false, Some(false)));
    assert_eq!(DeleteRole::ErrorDoNotDelete, delete_role_disambiguation(false, false, Some(true)));

    // User is author.
    // Deleted as a user.
    assert_eq!(DeleteRole::AsUser, delete_role_disambiguation(false, true, None));
    assert_eq!(DeleteRole::AsUser, delete_role_disambiguation(false, true, Some(false)));
    assert_eq!(DeleteRole::AsUser, delete_role_disambiguation(false, true, Some(true)));

    // User is mod.
    // Deleted as a mod.
    assert_eq!(DeleteRole::AsMod, delete_role_disambiguation(true, false, None));
    assert_eq!(DeleteRole::AsMod, delete_role_disambiguation(true, false, Some(false)));
    assert_eq!(DeleteRole::AsMod, delete_role_disambiguation(true, false, Some(true)));

    // User is mod and author.
    // Deleting as a user.
    assert_eq!(DeleteRole::AsUser, delete_role_disambiguation(true, true, Some(false)));

    // User is mod and author.
    // Deleting as a mod.
    assert_eq!(DeleteRole::AsMod, delete_role_disambiguation(true, true, None));
    assert_eq!(DeleteRole::AsMod, delete_role_disambiguation(true, true, Some(true)));
  }
}
