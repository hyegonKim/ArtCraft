use anyhow::anyhow;
use chrono::{DateTime, Utc};
use log::{info, warn};
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::helpers::boolean_converters::i8_to_bool;

#[derive(Serialize)]
pub struct UserRoleForList {
  pub slug: String,
  pub name: String,

  pub can_use_tts: bool,
  pub can_use_w2l: bool,
  pub can_delete_own_tts_results: bool,
  pub can_delete_own_w2l_results: bool,
  pub can_delete_own_account: bool,

  pub can_upload_tts_models: bool,
  pub can_upload_w2l_templates: bool,
  pub can_delete_own_tts_models: bool,
  pub can_delete_own_w2l_templates: bool,

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

  pub created_at : DateTime<Utc>,
  pub updated_at : DateTime<Utc>,
}

struct RawDbUserRoleForList {
  slug: String,
  name: String,

  can_use_tts: i8,
  can_use_w2l: i8,
  can_delete_own_tts_results: i8,
  can_delete_own_w2l_results: i8,
  can_delete_own_account: i8,

  can_upload_tts_models: i8,
  can_upload_w2l_templates: i8,
  can_delete_own_tts_models: i8,
  can_delete_own_w2l_templates: i8,

  can_approve_w2l_templates: i8,
  can_edit_other_users_profiles: i8,
  can_edit_other_users_tts_models: i8,
  can_edit_other_users_w2l_templates: i8,
  can_delete_other_users_tts_models: i8,
  can_delete_other_users_tts_results: i8,
  can_delete_other_users_w2l_templates: i8,
  can_delete_other_users_w2l_results: i8,
  can_ban_users: i8,
  can_delete_users: i8,

  created_at : DateTime<Utc>,
  updated_at : DateTime<Utc>,
}

pub async fn list_user_roles(
  mysql_pool: &MySqlPool,
) -> AnyhowResult<Vec<UserRoleForList>> {
  info!("listing user roles");
  let maybe_user_roles = sqlx::query_as!(
      RawDbUserRoleForList,
        r#"
SELECT
    slug,
    name,

    can_use_tts,
    can_use_w2l,
    can_delete_own_tts_results,
    can_delete_own_w2l_results,
    can_delete_own_account,

    can_upload_tts_models,
    can_upload_w2l_templates,
    can_delete_own_tts_models,
    can_delete_own_w2l_templates,

    can_approve_w2l_templates,
    can_edit_other_users_profiles,
    can_edit_other_users_tts_models,
    can_edit_other_users_w2l_templates,
    can_delete_other_users_tts_models,
    can_delete_other_users_tts_results,
    can_delete_other_users_w2l_templates,
    can_delete_other_users_w2l_results,
    can_ban_users,
    can_delete_users,

    created_at,
    updated_at

FROM user_roles
        "#,
      )
        .fetch_all(mysql_pool)
        .await;

  let user_roles : Vec<RawDbUserRoleForList> = match maybe_user_roles {
    Ok(roles) => roles,
    Err(err) => {
      warn!("Error: {:?}", err);
      match err {
        sqlx::Error::RowNotFound => Vec::new(),
        _ => {
          warn!("user role query error: {:?}", err);
          return Err(anyhow!("error querying user roles"));
        }
      }
    }
  };

  Ok(user_roles.into_iter()
      .map(|ur| {
        UserRoleForList {
          slug: ur.slug,
          name: ur.name,
          can_use_tts: i8_to_bool(ur.can_use_tts),
          can_use_w2l: i8_to_bool(ur.can_use_w2l),
          can_delete_own_tts_results: i8_to_bool(ur.can_delete_own_tts_results),
          can_delete_own_w2l_results: i8_to_bool(ur.can_delete_own_w2l_results),
          can_delete_own_account: i8_to_bool(ur.can_delete_own_account),
          can_upload_tts_models: i8_to_bool(ur.can_upload_tts_models),
          can_upload_w2l_templates: i8_to_bool(ur.can_upload_w2l_templates),
          can_delete_own_tts_models: i8_to_bool(ur.can_delete_own_tts_models),
          can_delete_own_w2l_templates: i8_to_bool(ur.can_delete_own_w2l_templates),
          can_approve_w2l_templates: i8_to_bool(ur.can_approve_w2l_templates),
          can_edit_other_users_profiles: i8_to_bool(ur.can_edit_other_users_profiles),
          can_edit_other_users_tts_models: i8_to_bool(ur.can_edit_other_users_tts_models),
          can_edit_other_users_w2l_templates: i8_to_bool(ur.can_edit_other_users_w2l_templates),
          can_delete_other_users_tts_models: i8_to_bool(ur.can_delete_other_users_tts_models),
          can_delete_other_users_tts_results: i8_to_bool(ur.can_delete_other_users_tts_results),
          can_delete_other_users_w2l_templates: i8_to_bool(ur.can_delete_other_users_w2l_templates),
          can_delete_other_users_w2l_results: i8_to_bool(ur.can_delete_other_users_w2l_results),
          can_ban_users: i8_to_bool(ur.can_ban_users),
          can_delete_users: i8_to_bool(ur.can_delete_users),
          created_at: ur.created_at,
          updated_at: ur.updated_at,
        }
      })
      .collect::<Vec<UserRoleForList>>())
}
