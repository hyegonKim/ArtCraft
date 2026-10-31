use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct CreateUserRoleArgs<'a> {
  // URL-friendly unique foreign key for the role
  pub slug: &'a str,

  // Human-friendly title of the role
  pub name: &'a str,

  // Usage

  pub can_use_tts: bool,
  pub can_use_w2l: bool,
  pub can_delete_own_tts_results: bool,
  pub can_delete_own_w2l_results: bool,
  pub can_delete_own_account: bool,

  // Contribution

  pub can_upload_tts_models: bool,
  pub can_upload_w2l_templates: bool,
  pub can_delete_own_tts_models: bool,
  pub can_delete_own_w2l_templates: bool,

  // Moderation

  pub can_approve_w2l_templates: bool,
  pub can_edit_other_users_profiles: bool,
  pub can_edit_other_users_tts_models: bool,
  pub can_edit_other_users_w2l_templates: bool,
  pub can_delete_other_users_tts_models: bool,
  pub can_delete_other_users_tts_results: bool,
  pub can_delete_other_users_w2l_templates: bool,
  pub can_delete_other_users_w2l_results: bool,
  pub can_ban_users: bool,
  pub can_delete_users: bool,

  pub mysql_pool: &'a MySqlPool
}

pub async fn insert_user_role(args: CreateUserRoleArgs<'_>) -> AnyhowResult<()> {
  let query_result = sqlx::query!(
        r#"
INSERT INTO user_roles
SET
  slug = ?,
  name = ?,

  can_use_tts = ?,
  can_use_w2l = ?,
  can_delete_own_tts_results = ?,
  can_delete_own_w2l_results = ?,
  can_delete_own_account = ?,

  can_upload_tts_models = ?,
  can_upload_w2l_templates = ?,
  can_delete_own_tts_models = ?,
  can_delete_own_w2l_templates = ?,

  can_approve_w2l_templates = ?,
  can_edit_other_users_profiles = ?,
  can_edit_other_users_tts_models = ?,
  can_edit_other_users_w2l_templates = ?,
  can_delete_other_users_tts_models = ?,
  can_delete_other_users_tts_results = ?,
  can_delete_other_users_w2l_templates = ?,
  can_delete_other_users_w2l_results = ?,
  can_ban_users = ?,
  can_delete_users = ?
        "#,
        args.slug,
        args.name,

        args.can_use_tts,
        args.can_use_w2l,
        args.can_delete_own_tts_results,
        args.can_delete_own_w2l_results,
        args.can_delete_own_account,

        args.can_upload_tts_models,
        args.can_upload_w2l_templates,
        args.can_delete_own_tts_models,
        args.can_delete_own_w2l_templates,

        args.can_approve_w2l_templates,
        args.can_edit_other_users_profiles,
        args.can_edit_other_users_tts_models,
        args.can_edit_other_users_w2l_templates,
        args.can_delete_other_users_tts_models,
        args.can_delete_other_users_tts_results,
        args.can_delete_other_users_w2l_templates,
        args.can_delete_other_users_w2l_results,
        args.can_ban_users,
        args.can_delete_users,
    )
      .execute(args.mysql_pool)
      .await;

  match query_result {
    Ok(_) => {},
    Err(err) => {
      return Err(anyhow!("user role creation error error: {:?}", err));
    }
  }

  Ok(())
}
