use clap::{Parser, ValueEnum};
use serde_derive::Deserialize;

use errors::AnyhowResult;

#[derive(Parser, Debug, Deserialize)]
#[command(name="db-cleanup")]
#[serde(rename_all = "snake_case")]
pub struct Args {
  // The primary action to take
  #[arg(name="action", long="action", help="the primary action to take", required=true)]
  pub action: Action,

  // Optional username for some actions
  #[arg(name="username", long="username", help="optional username", required=false)]
  pub username: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Action {
  /// Delete all anonymous user images.
  DeleteAllAnonymousUserImages,

  /// Delete files for a particular user.
  DeleteUserFiles,

  /// Migrate old media file enum values.
  MigrateMediaFilesEnumValues,
}

pub fn parse_cli_args() -> AnyhowResult<Args> {
  let mut args = Args::parse();

  if let Some(username) = args.username.as_deref() {
    args.username = Some(username.trim().to_string());
  }

  Ok(args)
}
