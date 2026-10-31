use sqlx;

use crate::utils::transactor::Transactor;
use errors::AnyhowResult;

pub struct UpdateGoogleSignInArgs<'e, 't> {
  pub subject: &'e str,

  pub email_address: &'e str,

  pub is_email_verified: bool,

  pub maybe_locale: Option<&'e str>,

  pub maybe_name: Option<&'e str>,
  pub maybe_given_name: Option<&'e str>,
  pub maybe_family_name: Option<&'e str>,

  pub creator_ip_address: &'e str,

  pub transactor: Transactor<'e, 't>,
}

pub async fn update_google_sign_in_account<'e, 't>(
  args: UpdateGoogleSignInArgs<'e, 't>
) -> AnyhowResult<()>
{
  let query = sqlx::query!(
      r#"
UPDATE google_sign_in_accounts
SET
  email_address = ?,
  is_email_verified = ?,

  maybe_locale = ?,

  maybe_name = ?,
  maybe_given_name = ?,
  maybe_family_name = ?,

  ip_address_update = ?,

  version = version + 1
WHERE
  subject = ?
        "#,
    args.email_address,
    args.is_email_verified,
    args.maybe_locale,
    args.maybe_name,
    args.maybe_given_name,
    args.maybe_family_name,
    args.creator_ip_address,
    args.subject,
  );

  let _r = args.transactor.execute(query).await?;

  Ok(())
}
