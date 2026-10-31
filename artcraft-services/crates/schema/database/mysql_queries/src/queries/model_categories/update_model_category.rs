use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct UpdateModelCategoryArgs<'a> {
  pub name: &'a str,
  pub maybe_dropdown_name: Option<&'a str>,
  pub can_directly_have_models: bool,
  pub can_have_subcategories: bool,
  pub can_only_mods_apply: bool,
  pub maybe_super_category_token: Option<&'a str>,
  pub is_mod_approved: bool,
  pub mod_user_token: &'a str,
  pub maybe_mod_comments: Option<&'a str>,
  pub model_category_token: &'a str,
  pub mysql_pool: &'a MySqlPool,
}

pub async fn update_model_category(args: UpdateModelCategoryArgs<'_>) -> AnyhowResult<()> {

  let query_result =
      // We need to store the IP address details.
      sqlx::query!(
        r#"
UPDATE model_categories
SET
    name = ?,
    maybe_dropdown_name = ?,

    can_directly_have_models = ?,
    can_have_subcategories = ?,
    can_only_mods_apply = ?,

    maybe_super_category_token = ?,

    is_mod_approved = ?,
    maybe_mod_user_token = ?,
    maybe_mod_comments = ?,

    version = version + 1

WHERE token = ?
LIMIT 1
        "#,
      args.name,
      args.maybe_dropdown_name,
      args.can_directly_have_models,
      args.can_have_subcategories,
      args.can_only_mods_apply,
      args.maybe_super_category_token,
      args.is_mod_approved,
      args.mod_user_token,
      args.maybe_mod_comments,
      args.model_category_token,
    )
          .execute(args.mysql_pool)
          .await;

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => {
      Err(anyhow!("Edit category DB error: {:?}", err))
    }
  }
}
