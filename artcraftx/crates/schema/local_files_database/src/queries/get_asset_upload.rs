use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;
use crate::queries::asset_upload_record::AssetUploadRecord;

pub struct GetAssetUploadArgs<'a> {
  pub db: &'a LocalFilesDbConnection,
  pub service: &'a str,
  pub account_id: &'a str,
  pub file_hash_blake3: &'a str,
}

/// The remote copy of this content on this service+account, if one was
/// recorded.
pub async fn get_asset_upload(args: GetAssetUploadArgs<'_>) -> Result<Option<AssetUploadRecord>, LocalFilesDbError> {
  let record = sqlx::query_as::<_, AssetUploadRecord>(
    r#"
    SELECT * FROM asset_uploads
    WHERE service = ?1 AND account_id = ?2 AND file_hash_blake3 = ?3
    "#,
  )
      .bind(args.service)
      .bind(args.account_id)
      .bind(args.file_hash_blake3)
      .fetch_optional(args.db.get_pool())
      .await?;
  Ok(record)
}
