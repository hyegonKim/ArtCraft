use anyhow::anyhow;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;

pub struct UnsubscribeReasonInsertBuilder<'a> {
  unsubscribe_reason: &'a str,
  user_token: &'a str,
  ip_address: &'a str,
}

impl<'a> UnsubscribeReasonInsertBuilder<'a> {
  pub fn new() -> Self {
    Self {
        unsubscribe_reason: "",
        user_token: "",
        ip_address: "",
    }
  }

  pub fn set_unsubscribe_reason(mut self, value: &'a str) -> Self {
    self.unsubscribe_reason = value;
    self
  }

  pub fn set_user_token(mut self, value: &'a str) -> Self {
    self.user_token = value;
    self
  }

  pub fn set_ip_address(mut self, value: &'a str) -> Self {
    self.ip_address = value;
    self
  }

  pub async fn insert(&mut self, mysql_connection: &mut PoolConnection<MySql>) -> AnyhowResult<()> {
    let query = sqlx::query!(
        r#"
INSERT INTO unsubscribe_reason 
SET 
    user_token = ?,
    feedback_reason = ?,
    ip_address = ?
        "#,
      self.user_token,
      self.unsubscribe_reason,
      self.ip_address,
      );

    let query_result = query.execute(&mut **mysql_connection)
        .await;

    let _record_id = match query_result {
      Ok(res) => {
        res.last_insert_id()
      },
      Err(err) => {
        return Err(anyhow!("Unsubscribe reason insert DB error: {:?}", err));
      }
    };

    Ok(())
  }
}
