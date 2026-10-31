use tokens::tokens::anonymous_visitor_tracking::AnonymousVisitorTrackingToken;
use tokens::tokens::users::UserToken;

/// This is used to determine if the caller has the ability to perform an action.
/// Files owned by users must be matched by the user token.
/// Files anonymously created must be matched by the AVT.
pub struct CheckCreatorTokenArgs<'a> {
  pub maybe_creator_user_token: Option<&'a UserToken>,
  pub maybe_current_request_user_token: Option<&'a UserToken>,
  pub maybe_creator_anonymous_visitor_token: Option<&'a AnonymousVisitorTrackingToken>,
  pub maybe_current_request_anonymous_visitor_token: Option<&'a AnonymousVisitorTrackingToken>,
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum CheckCreatorTokenResult {
  UserTokenMatch,
  UserTokenMismatch,
  NoUserAnonymousVisitorTokenMatch,
  NoUserAnonymousVisitorTokenMismatch,
  InsufficientInformation,
}

pub fn check_creator_tokens(args: CheckCreatorTokenArgs<'_>) -> CheckCreatorTokenResult {
  let maybe_user_tokens = (
    args.maybe_creator_user_token,
    args.maybe_current_request_user_token,
  );

  match maybe_user_tokens {
    (None, None) => {}, // Anonymous file and user; fall through
    (Some(token_a), Some(token_b)) => {
      if token_a == token_b {
        return CheckCreatorTokenResult::UserTokenMatch;
      } else {
        return CheckCreatorTokenResult::UserTokenMismatch;
      }
    },
    _ => {
      return CheckCreatorTokenResult::UserTokenMismatch;
    },
  }

  let maybe_anonymous_tokens = (
    args.maybe_creator_anonymous_visitor_token,
    args.maybe_current_request_anonymous_visitor_token,
  );

  match maybe_anonymous_tokens {
    (Some(token_a), Some(token_b)) => {
      if token_a == token_b {
        return CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMatch;
      } else {
        return CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMismatch;
      }
    },
    (None, None) => CheckCreatorTokenResult::InsufficientInformation, // No user info whatsoever.
    _ => {
      return CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMismatch;
    },
  }
}

#[cfg(test)]
mod tests {
  use crate::util::check_creator_tokens::{check_creator_tokens, CheckCreatorTokenArgs, CheckCreatorTokenResult};

  #[test]
  fn test_fully_anonymous() {
    let result = check_creator_tokens(CheckCreatorTokenArgs {
      maybe_creator_user_token: None,
      maybe_current_request_user_token: None,
      maybe_creator_anonymous_visitor_token: None,
      maybe_current_request_anonymous_visitor_token: None,
    });

    assert_eq!(result, CheckCreatorTokenResult::InsufficientInformation);
  }

  mod users {
    use tokens::tokens::users::UserToken;

    use super::*;

    #[test]
    fn test_same_user() {
      let user_1 = UserToken::new_from_str("bob");
      let user_2 = UserToken::new_from_str("bob");

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: Some(&user_1),
        maybe_current_request_user_token: Some(&user_2),
        maybe_creator_anonymous_visitor_token: None,
        maybe_current_request_anonymous_visitor_token: None,
      });

      assert_eq!(result, CheckCreatorTokenResult::UserTokenMatch);
    }

    #[test]
    fn test_different_user() {
      let user_1 = UserToken::new_from_str("bob");
      let user_2 = UserToken::new_from_str("jane");

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: Some(&user_1),
        maybe_current_request_user_token: Some(&user_2),
        maybe_creator_anonymous_visitor_token: None,
        maybe_current_request_anonymous_visitor_token: None,
      });

      assert_eq!(result, CheckCreatorTokenResult::UserTokenMismatch);
    }

    #[test]
    fn test_creator_user_versus_anonymous() {
      let user = Some(UserToken::new_from_str("jane"));

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: user.as_ref(), // As creator
        maybe_current_request_user_token: None,
        maybe_creator_anonymous_visitor_token: None,
        maybe_current_request_anonymous_visitor_token: None,
      });

      assert_eq!(result, CheckCreatorTokenResult::UserTokenMismatch);
    }

    #[test]
    fn test_anonymous_creator_vs_session_user() {
      let user = Some(UserToken::new_from_str("jane"));

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: None,
        maybe_current_request_user_token: user.as_ref(), // As session
        maybe_creator_anonymous_visitor_token: None,
        maybe_current_request_anonymous_visitor_token: None,
      });

      assert_eq!(result, CheckCreatorTokenResult::UserTokenMismatch);
    }
  }

  mod anonymous_tracking {
    use tokens::tokens::anonymous_visitor_tracking::AnonymousVisitorTrackingToken;

    use super::*;

    #[test]
    fn test_same_avt() {
      let anonymous_1 = AnonymousVisitorTrackingToken::new_from_str("anonymous_1");
      let anonymous_2 = AnonymousVisitorTrackingToken::new_from_str("anonymous_1");

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: None,
        maybe_current_request_user_token: None,
        maybe_creator_anonymous_visitor_token: Some(&anonymous_1),
        maybe_current_request_anonymous_visitor_token: Some(&anonymous_2),
      });

      assert_eq!(result, CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMatch);
    }

    #[test]
    fn test_avt_mismatch() {
      let anonymous_1 = AnonymousVisitorTrackingToken::new_from_str("anonymous_1");
      let anonymous_2 = AnonymousVisitorTrackingToken::new_from_str("anonymous_2");

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: None,
        maybe_current_request_user_token: None,
        maybe_creator_anonymous_visitor_token: Some(&anonymous_1),
        maybe_current_request_anonymous_visitor_token: Some(&anonymous_2),
      });

      assert_eq!(result, CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMismatch);
    }

    #[test]
    fn test_creator_avt_anonymous_user() {
      let anonymous = Some(AnonymousVisitorTrackingToken::new_from_str("anonymous"));

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: None,
        maybe_current_request_user_token: None,
        maybe_creator_anonymous_visitor_token: anonymous.as_ref(), // As creator
        maybe_current_request_anonymous_visitor_token: None,
      });

      assert_eq!(result, CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMismatch);
    }

    #[test]
    fn test_no_creator_anonymous_session_user() {
      let anonymous = Some(AnonymousVisitorTrackingToken::new_from_str("anonymous"));

      let result = check_creator_tokens(CheckCreatorTokenArgs {
        maybe_creator_user_token: None,
        maybe_current_request_user_token: None,
        maybe_creator_anonymous_visitor_token: None,
        maybe_current_request_anonymous_visitor_token: anonymous.as_ref(),
      });

      assert_eq!(result, CheckCreatorTokenResult::NoUserAnonymousVisitorTokenMismatch);
    }
  }
}
