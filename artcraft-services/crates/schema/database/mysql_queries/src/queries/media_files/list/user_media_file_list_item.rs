use chrono::{DateTime, Utc};
use sqlx::{FromRow, Row};
use sqlx::mysql::MySqlRow;

use enums::by_table::media_files::media_file_animation_type::MediaFileAnimationType;
use enums::traits::mysql_from_row::MySqlFromRow as _;
use enums::by_table::media_files::media_file_class::MediaFileClass;
use enums::by_table::media_files::media_file_engine_category::MediaFileEngineCategory;
use enums::by_table::media_files::media_file_origin_category::MediaFileOriginCategory;
use enums::by_table::media_files::media_file_origin_model_type::MediaFileOriginModelType;
use enums::by_table::media_files::media_file_origin_product_category::MediaFileOriginProductCategory;
use enums::by_table::media_files::media_file_type::MediaFileType;
use enums::common::visibility::Visibility;
use tokens::tokens::media_files::MediaFileToken;
use tokens::tokens::prompts::PromptToken;
use tokens::traits::mysql_token_from_row::MySqlTokenFromRow;
use crate::helpers::boolean_converters::i8_to_bool;
use crate::payloads::prompt_args::prompt_inner_payload::PromptInnerPayload;

pub struct MediaFileListItem {
  pub token: MediaFileToken,

  pub media_class: MediaFileClass,
  pub media_type: MediaFileType,

  pub maybe_engine_category: Option<MediaFileEngineCategory>,
  pub maybe_animation_type: Option<MediaFileAnimationType>,

  pub origin_category: MediaFileOriginCategory,
  pub origin_product_category: MediaFileOriginProductCategory,

  pub maybe_origin_model_type: Option<MediaFileOriginModelType>,
  pub maybe_origin_model_token: Option<String>,

  // NB: The title won't be populated for `tts_models` records or non-`model_weights` records.
  pub maybe_origin_model_title: Option<String>,

  pub public_bucket_directory_hash: String,
  pub maybe_public_bucket_prefix: Option<String>,
  pub maybe_public_bucket_extension: Option<String>,
  pub maybe_thumbnail_version: Option<u8>,

  pub maybe_prompt_token: Option<PromptToken>,

  pub creator_set_visibility: Visibility,

  pub is_user_upload: bool,
  pub is_intermediate_system_file: bool,

  pub maybe_file_cover_image_public_bucket_hash: Option<String>,
  pub maybe_file_cover_image_public_bucket_prefix: Option<String>,
  pub maybe_file_cover_image_public_bucket_extension: Option<String>,

  pub maybe_title: Option<String>,

  /// Text transcripts for TTS, etc.
  pub maybe_text_transcript: Option<String>,
  pub maybe_prompt_args: Option<PromptInnerPayload>,

  pub maybe_duration_millis: Option<u64>,

  pub maybe_ratings_positive_count: Option<u32>,
  pub maybe_ratings_negative_count: Option<u32>,
  pub maybe_bookmark_count: Option<u32>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

pub(super) struct MediaFileListItemInternal {
  id: i64,
  token: MediaFileToken,

  media_class: MediaFileClass,
  media_type: MediaFileType,

  maybe_engine_category: Option<MediaFileEngineCategory>,
  maybe_animation_type: Option<MediaFileAnimationType>,

  origin_category: MediaFileOriginCategory,
  origin_product_category: MediaFileOriginProductCategory,

  maybe_origin_model_type: Option<MediaFileOriginModelType>,
  maybe_origin_model_token: Option<String>,

  // NB: The title won't be populated for `tts_models` records or non-`model_weights` records.
  maybe_origin_model_title: Option<String>,
  maybe_other_prompt_args: Option<String>,
  maybe_duration_millis: Option<i32>,

  public_bucket_directory_hash: String,
  maybe_public_bucket_prefix: Option<String>,
  maybe_public_bucket_extension: Option<String>,
  maybe_thumbnail_version: Option<u8>,

  maybe_prompt_token: Option<PromptToken>,

  creator_set_visibility: Visibility,

  is_user_upload: i8,
  is_intermediate_system_file: i8,

  maybe_file_cover_image_public_bucket_hash: Option<String>,
  maybe_file_cover_image_public_bucket_prefix: Option<String>,
  maybe_file_cover_image_public_bucket_extension: Option<String>,

  maybe_title: Option<String>,

  /// Text transcripts for TTS, etc.
  maybe_text_transcript: Option<String>,

  maybe_ratings_positive_count: Option<u32>,
  maybe_ratings_negative_count: Option<u32>,
  maybe_bookmark_count: Option<u32>,

  created_at: DateTime<Utc>,
  updated_at: DateTime<Utc>,
}

// NB(bt,2023-12-05): There's an issue with type hinting in the `as` clauses with QueryBuilder (or
// raw query strings) and sqlx::FromRow, regardless of whether it is derived of manually
// implemented. Perhaps this will improve in the future, but for now manually constructed queries
// cannot have type hints, eg. the following:
//
//    m.token as `token: tokens::tokens::media_files::MediaFileToken`,
//    m.origin_category as `origin_category: enums::by_table::media_files::media_file_origin_category::MediaFileOriginCategory`,
//    m.creator_set_visibility as `creator_set_visibility: enums::common::visibility::Visibility`,
//
// This results in the automatic mapping not being able to be found by name (for macro derive), and
// in the manual case `row.try_get()` etc. won't have the correct column name (since the name is the
// full "as" clause).
impl FromRow<'_, MySqlRow> for MediaFileListItemInternal {
  fn from_row(row: &MySqlRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      id: row.try_get("id")?,
      token: MediaFileToken::new(row.try_get("token")?),
      media_class: MediaFileClass::try_from_mysql_row(row, "media_class")?,
      media_type: MediaFileType::try_from_mysql_row(row, "media_type")?,
      maybe_engine_category: MediaFileEngineCategory::try_from_mysql_row_nullable(row, "maybe_engine_category")?,
      maybe_animation_type: MediaFileAnimationType::try_from_mysql_row_nullable(row, "maybe_animation_type")?,
      origin_category: MediaFileOriginCategory::try_from_mysql_row(row, "origin_category")?,
      origin_product_category: MediaFileOriginProductCategory::try_from_mysql_row(row, "origin_product_category")?,
      maybe_origin_model_type: MediaFileOriginModelType::try_from_mysql_row_nullable(row, "maybe_origin_model_type")?,
      maybe_origin_model_token: row.try_get("maybe_origin_model_token")?,
      maybe_origin_model_title: row.try_get("maybe_origin_model_title")?,
      maybe_other_prompt_args: row.try_get("maybe_other_prompt_args")?,
      maybe_duration_millis: row.try_get("maybe_duration_millis")?,
      public_bucket_directory_hash: row.try_get("public_bucket_directory_hash")?,
      maybe_public_bucket_prefix: row.try_get("maybe_public_bucket_prefix")?,
      maybe_public_bucket_extension: row.try_get("maybe_public_bucket_extension")?,
      maybe_thumbnail_version: row.try_get("maybe_thumbnail_version")?,
      maybe_prompt_token: PromptToken::try_from_mysql_row_nullable(row, "maybe_prompt_token")?,
      creator_set_visibility: Visibility::try_from_mysql_row(row, "creator_set_visibility")?,
      is_user_upload: row.try_get("is_user_upload")?,
      is_intermediate_system_file: row.try_get("is_intermediate_system_file")?,
      maybe_file_cover_image_public_bucket_hash: row.try_get("maybe_file_cover_image_public_bucket_hash")?,
      maybe_file_cover_image_public_bucket_prefix: row.try_get("maybe_file_cover_image_public_bucket_prefix")?,
      maybe_file_cover_image_public_bucket_extension: row.try_get("maybe_file_cover_image_public_bucket_extension")?,
      maybe_title: row.try_get("maybe_title")?,
      maybe_text_transcript: row.try_get("maybe_text_transcript")?,
      maybe_ratings_positive_count: row.try_get("maybe_ratings_positive_count")?,
      maybe_ratings_negative_count: row.try_get("maybe_ratings_negative_count")?,
      maybe_bookmark_count: row.try_get("maybe_bookmark_count")?,
      created_at: row.try_get("created_at")?,
      updated_at: row.try_get("updated_at")?,
    })
  }
}

impl From<MediaFileListItemInternal> for MediaFileListItem {
  fn from(record: MediaFileListItemInternal) -> Self {
    MediaFileListItem {
      token: record.token,
      media_class: record.media_class,
      media_type: record.media_type,
      maybe_engine_category: record.maybe_engine_category,
      maybe_animation_type: record.maybe_animation_type,
      origin_category: record.origin_category,
      origin_product_category: record.origin_product_category,
      maybe_origin_model_type: record.maybe_origin_model_type,
      maybe_origin_model_token: record.maybe_origin_model_token,
      maybe_origin_model_title: record.maybe_origin_model_title,
      public_bucket_directory_hash: record.public_bucket_directory_hash,
      maybe_public_bucket_prefix: record.maybe_public_bucket_prefix,
      maybe_public_bucket_extension: record.maybe_public_bucket_extension,
      maybe_thumbnail_version: record.maybe_thumbnail_version,
      maybe_prompt_token: record.maybe_prompt_token,
      creator_set_visibility: record.creator_set_visibility,
      is_user_upload: i8_to_bool(record.is_user_upload),
      is_intermediate_system_file: i8_to_bool(record.is_intermediate_system_file),
      maybe_file_cover_image_public_bucket_hash: record.maybe_file_cover_image_public_bucket_hash,
      maybe_file_cover_image_public_bucket_prefix: record.maybe_file_cover_image_public_bucket_prefix,
      maybe_file_cover_image_public_bucket_extension: record.maybe_file_cover_image_public_bucket_extension,
      maybe_title: record.maybe_title,
      maybe_text_transcript: record.maybe_text_transcript,
      maybe_prompt_args: record.maybe_other_prompt_args
          .as_deref()
          .map(|args| PromptInnerPayload::from_json(args))
          .transpose()
          .ok() // NB: Fail open
          .flatten(),
      maybe_duration_millis: record.maybe_duration_millis.map(|d| d as u64),
      maybe_ratings_positive_count: record.maybe_ratings_positive_count,
      maybe_ratings_negative_count: record.maybe_ratings_negative_count,
      maybe_bookmark_count: record.maybe_bookmark_count,
      created_at: record.created_at,
      updated_at: record.updated_at,
    }
  }
}
