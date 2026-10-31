use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct CreateCategoryArgs<'a> {
  pub category_token: &'a str,
  pub idempotency_token: &'a str,
  pub model_type: &'a str,
  pub name: &'a str,
  pub creator_user_token: &'a str,
  pub creator_ip_address: &'a str,
  pub is_mod_approved: bool,
  pub maybe_mod_user_token: Option<&'a str>,
  pub can_directly_have_models: bool,
  pub can_have_subcategories: bool,
  pub can_only_mods_apply: bool,
  pub mysql_pool: &'a MySqlPool
}

pub async fn create_category(args: CreateCategoryArgs<'_>) -> AnyhowResult<()> {
  let query_result = sqlx::query!(
        r#"
INSERT INTO model_categories
SET
    token = ?,
    uuid_idempotency_token = ?,
    model_type = ?,
    name = ?,

    creator_user_token = ?,
    creator_ip_address_creation = ?,
    creator_ip_address_last_update = ?,

    is_mod_approved = ?,
    maybe_mod_user_token = ?,
    can_directly_have_models = ?,
    can_have_subcategories = ?,
    can_only_mods_apply = ?
        "#,

    args.category_token,
    args.idempotency_token,
    args.model_type,
    args.name,
    args.creator_user_token,
    args.creator_ip_address,
    args.creator_ip_address,
    args.is_mod_approved,
    args.maybe_mod_user_token,
    args.can_directly_have_models,
    args.can_have_subcategories,
    args.can_only_mods_apply
  )
      .execute(args.mysql_pool)
      .await;

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => Err(anyhow!("model category creation error: {:?}", err)),
  }
}
