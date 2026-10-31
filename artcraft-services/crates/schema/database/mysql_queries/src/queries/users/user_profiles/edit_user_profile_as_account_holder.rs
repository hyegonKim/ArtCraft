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

  pub preferred_tts_result_visibility: &'a str,
  pub preferred_w2l_result_visibility: &'a str,

  // We need to store the IP address details for non-mods.
  pub ip_address: &'a str,
}

pub async fn edit_user_profile_as_account_holder(
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
    preferred_tts_result_visibility = ?,
    preferred_w2l_result_visibility = ?,
    discord_username = ?,
    twitter_username = ?,
    twitch_username = ?,
    github_username = ?,
    cashapp_username = ?,
    website_url = ?,
    ip_address_last_update = ?,
    version = version + 1

WHERE users.token = ?
LIMIT 1
        "#,
      args.profile_markdown,
      args.profile_html,
      args.preferred_tts_result_visibility,
      args.preferred_w2l_result_visibility,
      args.discord_username,
      args.twitter_username,
      args.twitch_username,
      args.github_username,
      args.cashapp_username,
      args.website_url,
      args.ip_address,
      args.user_token,
    )
      .execute(mysql_pool)
      .await?;

  Ok(())
}

