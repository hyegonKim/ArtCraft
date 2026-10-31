use log::{info, warn};
use sqlx::{MySql, Pool};

use errors::AnyhowResult;
use mysql_queries::queries::users::user_roles::insert_user_role::{CreateUserRoleArgs, insert_user_role};

pub async fn seed_user_roles(mysql_pool: &Pool<MySql>) -> AnyhowResult<()> {

  let roles = [
    ("user", "User", true, false, false),
    ("mod", "Moderator", true, true, true),
    ("admin", "Admin", true, true, true),
  ];

  for (slug, name, can_use, can_contribute, can_moderate) in roles {
    let result = create_user_role(slug, name, can_use, can_contribute, can_moderate, mysql_pool).await;
    match result {
      Ok(_) => info!("Seeded user role: {}", slug),
      Err(err) => warn!(r#"
        Could not seed user role {} : {:?}
        (No worries: if there's a duplicate key error, we probably already
        seeded the user role on a previous invocation!)
      "#, slug, err),
    }
  }

  Ok(())
}

pub async fn create_user_role(
  slug: &str,
  name: &str,
  can_use: bool,
  can_contribute: bool,
  can_moderate: bool,
  mysql_pool: &Pool<MySql>,
) -> AnyhowResult<()> {

  let _result = insert_user_role(CreateUserRoleArgs {
    slug,
    name,
    // Use
    can_use_tts: can_use,
    can_use_w2l: can_use,
    can_delete_own_tts_results: can_use,
    can_delete_own_w2l_results: can_use,
    can_delete_own_account: can_use,
    // Contribute
    can_upload_tts_models: can_contribute,
    can_upload_w2l_templates: can_contribute,
    can_delete_own_tts_models: can_contribute,
    can_delete_own_w2l_templates: can_contribute,
    // Moderate
    can_approve_w2l_templates: can_moderate,
    can_edit_other_users_profiles: can_moderate,
    can_edit_other_users_tts_models: can_moderate,
    can_edit_other_users_w2l_templates: can_moderate,
    can_delete_other_users_tts_models: can_moderate,
    can_delete_other_users_tts_results: can_moderate,
    can_delete_other_users_w2l_templates: can_moderate,
    can_delete_other_users_w2l_results: can_moderate,
    can_ban_users: can_moderate,
    can_delete_users: can_moderate,
    mysql_pool,
  }).await?;

  Ok(())
}
