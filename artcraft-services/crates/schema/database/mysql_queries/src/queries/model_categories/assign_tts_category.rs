use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Copy, Clone)]
pub enum AssignOrDeleteAction {
  CreateAssignment,
  DeleteAssignment,
}

pub struct AssignTtsCategoryArgs<'a> {
  pub tts_model_token: &'a str,
  pub tts_category_token: &'a str,

  pub editor_user_token: &'a str,
  pub editor_ip_address: &'a str,

  pub action: AssignOrDeleteAction,

  pub mysql_pool: &'a MySqlPool,
}

pub async fn assign_tts_category(args: AssignTtsCategoryArgs<'_>) -> AnyhowResult<()> {

  let query_builder = match args.action {
    AssignOrDeleteAction::CreateAssignment => {
      // NB: deleted_at = NULL
      sqlx::query!(r#"
      INSERT INTO tts_category_assignments
      SET
        model_token = ?,
        category_token = ?,
        category_addition_user_token = ?,
        ip_address_creation = ?,
        ip_address_last_update = ?,
        deleted_at = NULL
      ON DUPLICATE KEY UPDATE
        model_token = ?,
        category_token = ?,
        category_addition_user_token = ?,
        ip_address_last_update = ?,
        deleted_at = NULL
      "#,
        // Insert
        args.tts_model_token,
        args.tts_category_token,
        args.editor_user_token,
        args.editor_ip_address,
        args.editor_ip_address,
        // Update
        args.tts_model_token,
        args.tts_category_token,
        args.editor_user_token,
        args.editor_ip_address,
      )
    },
    AssignOrDeleteAction::DeleteAssignment => {
      // NB: deleted_at = CURRENT_TIMESTAMP
      sqlx::query!(r#"
      INSERT INTO tts_category_assignments
      SET
        model_token = ?,
        category_token = ?,
        category_removal_user_token = ?,
        ip_address_creation = ?,
        ip_address_last_update = ?,
        deleted_at = CURRENT_TIMESTAMP
      ON DUPLICATE KEY UPDATE
        model_token = ?,
        category_token = ?,
        category_removal_user_token = ?,
        ip_address_last_update = ?,
        deleted_at = CURRENT_TIMESTAMP
      "#,
        // Insert
        args.tts_model_token,
        args.tts_category_token,
        args.editor_user_token,
        args.editor_ip_address,
        args.editor_ip_address,
        // Update
        args.tts_model_token,
        args.tts_category_token,
        args.editor_user_token,
        args.editor_ip_address,
      )
    }
  };

  // NB: We're soft deleting so we don't delete the associations.
  let query_result = query_builder.execute(args.mysql_pool).await;

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => Err(anyhow!("Assign category edit DB error: {:?}", err)),
  }
}
