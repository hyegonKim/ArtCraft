use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::media_files::MediaFileToken;

pub struct UpdateArgs<'a> {
  pub media_file_token: &'a MediaFileToken,
  pub maybe_cover_image_media_file_token: Option<&'a MediaFileToken>,
  //pub update_ip_address: &'a str,
  pub mysql_pool: &'a MySqlPool
}

pub async fn set_media_file_cover_image(args: UpdateArgs<'_>) -> AnyhowResult<()>{
  let transaction = args.mysql_pool.begin().await?;

  let _query_result = sqlx::query!(
        r#"
        UPDATE media_files
        SET
            maybe_cover_image_media_file_token = ?
        WHERE token = ?
        LIMIT 1
        "#,
        args.maybe_cover_image_media_file_token,
        args.media_file_token,
    ).execute(args.mysql_pool).await?;

  transaction.commit().await?;

  Ok(())
}
