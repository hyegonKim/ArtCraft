use tokens::tokens::news_stories::NewsStoryToken;

pub struct NewsStoryListItem {
  pub news_story_token: NewsStoryToken,

  pub original_news_canonical_url: String,
  pub original_news_title: String,
  pub summary_news_title: String,

  pub audio_file_count: i64,
  pub audio_total_duration_seconds: i64,
}
