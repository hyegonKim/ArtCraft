use anyhow::anyhow;
use chrono::{DateTime, Utc};
use log::warn;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::helpers::boolean_converters::{i8_to_bool, nullable_i8_to_optional_bool};

#[derive(Serialize)]
pub struct ModelCategory {
  // Non-included fields:
  //pub id: i64,
  //pub uuid_idempotency_token: String,
  //pub version: i64,

  pub category_token: String,
  pub model_type: String, // TODO: enum
  pub maybe_super_category_token: Option<String>,

  pub can_directly_have_models: bool,
  pub can_have_subcategories: bool,
  pub can_only_mods_apply: bool,

  pub name: String,
  pub maybe_dropdown_name: Option<String>,

  pub creator_user_token: String,
  pub creator_username: String,
  pub creator_display_name: String,
  pub creator_gravatar_hash: String,

  pub creator_ip_address_creation: String,
  pub creator_ip_address_last_update: String,

  pub is_mod_approved: Option<bool>,

  pub maybe_mod_comments: Option<String>,
  pub maybe_mod_user_token: Option<String>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub deleted_at: Option<DateTime<Utc>>,
}

pub async fn get_category_by_token(
  category_token: &str,
  mysql_pool: &MySqlPool
) -> AnyhowResult<Option<ModelCategory>> {
  let maybe_category_record = sqlx::query_as!(
      RawModelCategory,
        r#"
SELECT
    category.token as category_token,
    category.model_type,
    category.maybe_super_category_token,
    category.can_directly_have_models,
    category.can_have_subcategories,
    category.can_only_mods_apply,
    category.name,
    category.maybe_dropdown_name,
    category.creator_user_token,
    users.username as creator_username,
    users.display_name as creator_display_name,
    users.email_gravatar_hash AS creator_gravatar_hash,
    category.creator_ip_address_creation,
    category.creator_ip_address_last_update,
    category.is_mod_approved,
    category.maybe_mod_comments,
    category.maybe_mod_user_token,
    category.created_at,
    category.updated_at,
    category.deleted_at
FROM model_categories AS category
JOIN users
ON category.creator_user_token = users.token
WHERE
    category.token = ?
        "#,
        category_token,
    )
      .fetch_one(mysql_pool)
      .await;

  let category : RawModelCategory = match maybe_category_record {
    Ok(category) => category,
    Err(err) => {
      return match err {
        sqlx::Error::RowNotFound => {
          Ok(None)
        },
        _ => {
          warn!("User profile query error: {:?}", err);
          Err(anyhow!("query error"))
        }
      }
    }
  };

  let category = ModelCategory {
    category_token: category.category_token,
    model_type: category.model_type,
    maybe_super_category_token: category.maybe_super_category_token,
    can_directly_have_models: i8_to_bool(category.can_directly_have_models),
    can_have_subcategories: i8_to_bool(category.can_have_subcategories),
    can_only_mods_apply: i8_to_bool(category.can_only_mods_apply),
    name: category.name.clone(),
    maybe_dropdown_name: category.maybe_dropdown_name,
    creator_user_token: category.creator_user_token,
    creator_username: category.creator_username,
    creator_display_name: category.creator_display_name,
    creator_gravatar_hash: category.creator_gravatar_hash,
    creator_ip_address_creation: category.creator_ip_address_creation,
    creator_ip_address_last_update: category.creator_ip_address_last_update,
    is_mod_approved: nullable_i8_to_optional_bool(category.is_mod_approved),
    maybe_mod_comments: category.maybe_mod_comments,
    maybe_mod_user_token: category.maybe_mod_user_token,
    created_at: category.created_at,
    updated_at: category.updated_at,
    deleted_at: category.deleted_at,
  };

  Ok(Some(category))
}

#[derive(Serialize)]
pub struct RawModelCategory {
  // Non-included fields:
  //pub id: i64,
  //pub uuid_idempotency_token: String,
  //pub version: i64,

  pub category_token: String,
  pub model_type: String, // TODO: enum
  pub maybe_super_category_token: Option<String>,

  pub can_directly_have_models: i8,
  pub can_have_subcategories: i8,
  pub can_only_mods_apply: i8,

  pub name: String,
  pub maybe_dropdown_name: Option<String>,

  pub creator_user_token: String,
  pub creator_username: String,
  pub creator_display_name: String,
  pub creator_gravatar_hash: String,

  pub creator_ip_address_creation: String,
  pub creator_ip_address_last_update: String,

  pub is_mod_approved: Option<i8>,

  pub maybe_mod_comments: Option<String>,
  pub maybe_mod_user_token: Option<String>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub deleted_at: Option<DateTime<Utc>>,
}
