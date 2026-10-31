use anyhow::anyhow;
use chrono::{DateTime, Utc};
use log::{info, warn};
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;

#[derive(Serialize, Deserialize, Clone)]
pub struct UserBadgeForList {
  pub slug: String,
  pub title: String,
  pub description: String,
  pub image_url: String,
  pub granted_at: DateTime<Utc>,
}

struct RawDbUserBadgeForList {
  slug: String,
  title: String,
  description: String,
  image_url: String,
  user_created_at : DateTime<Utc>,
}

pub async fn list_user_badges(
  mysql_connector: &mut PoolConnection<MySql>,
  user_token: &str,
) -> AnyhowResult<Vec<UserBadgeForList>> {
  info!("listing user badges");
  let maybe_user_badges = sqlx::query_as!(
      RawDbUserBadgeForList,
        r#"
SELECT
    badges.slug,
    badges.title,
    badges.description,
    badges.image_url,
    user_badges.created_at as user_created_at

FROM badges
JOIN user_badges
ON
    badges.slug = user_badges.badge_slug
WHERE
    user_badges.user_token = ?
        "#,
        user_token
      )
      .fetch_all(&mut **mysql_connector)
      .await;

  let user_badges : Vec<RawDbUserBadgeForList> = match maybe_user_badges {
    Ok(badges) => badges,
    Err(err) => {
      warn!("Error: {:?}", err);
      match err {
        sqlx::Error::RowNotFound => Vec::new(),
        _ => {
          warn!("user badges query error: {:?}", err);
          return Err(anyhow!("error querying user badges"));
        }
      }
    }
  };

  Ok(user_badges.into_iter()
      .map(|badge| {
        UserBadgeForList {
          slug: badge.slug,
          title: badge.title,
          description: badge.description,
          image_url: badge.image_url,
          granted_at: badge.user_created_at,
        }
      })
      .collect::<Vec<UserBadgeForList>>())
}
