use anyhow::Context;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::password_reset::PasswordResetToken;

use crate::queries::users::user::get::lookup_user_for_login_result::UserRecordForLogin;

pub async fn create_password_reset(pool: &MySqlPool, user: &UserRecordForLogin, ip_address: &str, secret_key: String) -> AnyhowResult<()> {
    let token = PasswordResetToken::generate();

    sqlx::query!(
        r#"
INSERT INTO user_password_resets
(token, user_token, public_reset_token, current_password_version, ip_address_creation, expires_at)
VALUES (?, ?, ?, ?, ?, NOW() + INTERVAL 3 hour);
        "#,

        token,
        user.token,
        secret_key,
        user.password_version,
        ip_address
    )
    .execute(pool)
    .await
    .context("inserting password reset")
    .map(|_| ())
}
