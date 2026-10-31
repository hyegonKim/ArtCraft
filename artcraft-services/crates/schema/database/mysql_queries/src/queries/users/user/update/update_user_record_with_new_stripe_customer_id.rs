use anyhow::anyhow;
use sqlx::{MySql, MySqlPool};
use sqlx::pool::PoolConnection;
use errors::AnyhowResult;

pub async fn update_user_record_with_new_stripe_customer_id(
    mysql_pool: &MySqlPool,
    user_token: &str,
    maybe_stripe_customer_id: Option<&str>
) -> AnyhowResult<()> {
    let mut mysql_connection = mysql_pool.acquire().await?;
    update_user_record_with_new_stripe_customer_id_with_connection(
        &mut mysql_connection,
        user_token,
        maybe_stripe_customer_id
    ).await
}

pub async fn update_user_record_with_new_stripe_customer_id_with_connection(
    mysql_connection: &mut PoolConnection<MySql>,
    user_token: &str,
    maybe_stripe_customer_id: Option<&str>
) -> AnyhowResult<()> {

    // TODO: This will overwrite whatever the previous customer_id was.
    //  Should we guard against that?

    let query = sqlx::query!(
        r#"
UPDATE users
SET
  maybe_stripe_customer_id = ?,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
        maybe_stripe_customer_id,
        user_token,
    );

    let query_result = query.execute(&mut ** mysql_connection).await;

    match query_result {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow!("Error creating stripe webhook event log: {:?}", err)),
    }
}
