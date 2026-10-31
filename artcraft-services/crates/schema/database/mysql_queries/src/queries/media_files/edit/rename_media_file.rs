use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::media_files::MediaFileToken;

pub async fn rename_media_file(
    media_file_token: &MediaFileToken,
    maybe_name: Option<&str>,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let new_name = maybe_name.unwrap_or("").trim();

    if new_name.is_empty() {
        clear_media_file_name(media_file_token, mysql_pool).await
    } else {
        set_media_file_name(media_file_token, new_name, mysql_pool).await
    }
}

async fn clear_media_file_name(
    media_file_token: &MediaFileToken,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE media_files
SET
  maybe_title = NULL
WHERE
  token = ?
LIMIT 1
        "#,
      media_file_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn set_media_file_name(
    media_file_token: &MediaFileToken,
    name: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE media_files
SET
  maybe_title = ?
WHERE
  token = ?
LIMIT 1
        "#,
      name,
      media_file_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}