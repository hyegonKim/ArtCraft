use errors::{anyhow, AnyhowResult};
use log::{error, warn};
use sqlx::SqlitePool;
use tokens::tokens::news_stories::NewsStoryToken;

pub struct NewsStoryItem {
  pub news_story_token: NewsStoryToken,

  pub original_news_canonical_url: String,
  pub original_news_title: String,

  pub audio_file_count: i64,
  pub audio_total_duration_seconds: i64,
}

pub async fn get_news_story_by_token(
  sqlite_pool: &SqlitePool,
  news_story_token: &NewsStoryToken,
) -> AnyhowResult<Option<NewsStoryItem>> {

  let token = news_story_token.to_string();

  let query = sqlx::query_as!(
    NewsStoryItem,
        r#"
SELECT
  news_story_token as `news_story_token: tokens::tokens::news_stories::NewsStoryToken`,
  original_news_canonical_url,
  original_news_title,
  audio_file_count,
  audio_total_duration_seconds
FROM news_stories
WHERE
  news_story_token = ?
        "#,
      token
    );

  let maybe_record = query.fetch_one(sqlite_pool)
      .await;

  match maybe_record {
    Err(ref err) => {
      match err {
        sqlx::Error::RowNotFound => {
          warn!("record not found: {:?}", &err);
          return Ok(None);
        },
        _ => {
          error!("query error: {:?}", &err);
          return Err(anyhow!("database error"));
        }
      }
    }
    Ok(record) => Ok(Some(record)),
  }
}
