use sqlx;
use sqlx::{Executor, MySql, Transaction};

use errors::AnyhowResult;
use tokens::tokens::google_sign_in_accounts::GoogleSignInAccountToken;
use tokens::tokens::users::UserToken;

pub struct InsertGoogleSignInArgs<'e, 't> {
  pub subject: &'e str,

  pub maybe_user_token: Option<&'e UserToken>,

  pub email_address: &'e str,

  pub is_email_verified: bool,

  pub maybe_locale: Option<&'e str>,

  pub maybe_name: Option<&'e str>,
  pub maybe_given_name: Option<&'e str>,
  pub maybe_family_name: Option<&'e str>,

  pub creator_ip_address: &'e str,

  pub transaction: &'e mut Transaction<'t, MySql>,
}

pub async fn insert_google_sign_in_account<'e, 't>(
  args: InsertGoogleSignInArgs<'e, 't>
) -> AnyhowResult<GoogleSignInAccountToken>
{
  let token = GoogleSignInAccountToken::generate();

  let query = sqlx::query!(
      r#"
INSERT INTO google_sign_in_accounts
SET
  token = ?,
  subject = ?,

  maybe_user_token = ?,

  email_address = ?,
  is_email_verified = ?,

  maybe_locale = ?,

  maybe_name = ?,
  maybe_given_name = ?,
  maybe_family_name = ?,

  ip_address_creation = ?,
  ip_address_update = ?
        "#,
    token.as_str(),
    args.subject,
    args.maybe_user_token.map(|t| t.as_str()),
    args.email_address,
    args.is_email_verified,
    args.maybe_locale,
    args.maybe_name,
    args.maybe_given_name,
    args.maybe_family_name,
    args.creator_ip_address,
    args.creator_ip_address,
  );

  let _r = query.execute(&mut **args.transaction).await?;

  Ok(token)
}
