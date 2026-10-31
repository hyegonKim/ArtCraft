// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct Args<'a> {
  pub user_token: &'a str,

  pub profile_markdown: Option<&'a str>,
  pub profile_html: Option<&'a str>,

  pub discord_username: Option<&'a str>,
  pub twitter_username: Option<&'a str>,
  pub cashapp_username: Option<&'a str>,
  pub github_username: Option<&'a str>,
  pub twitch_username: Option<&'a str>,
  pub website_url: Option<&'a str>,
}

/// Mods can edit user profiles, but shouldn't be able to change preferences
/// We won't set the last edit IP address either.
pub async fn edit_user_profile_as_mod(
  mysql_pool: &MySqlPool,
  args: Args<'_>,
) -> AnyhowResult<()>
{
  let _result = sqlx::query!(
        r#"
UPDATE users
SET
    profile_markdown = ?,
    profile_rendered_html = ?,
    discord_username = ?,
    twitter_username = ?,
    twitch_username = ?,
    github_username = ?,
    cashapp_username = ?,
    website_url = ?,
    version = version + 1

WHERE users.token = ?
LIMIT 1
        "#,
      args.profile_markdown,
      args.profile_html,
      args.discord_username,
      args.twitter_username,
      args.twitch_username,
      args.github_username,
      args.cashapp_username,
      args.website_url,
      args.user_token,
    )
      .execute(mysql_pool)
      .await?;

  Ok(())
}
