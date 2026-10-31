use std::str::FromStr;

use clap::Parser;
use strum::{EnumCount, EnumString, IntoEnumIterator};
use strum::EnumIter;

use errors::{anyhow, AnyhowResult};

pub struct ParsedArgs {
  pub action: Action,
}

#[derive(Clone, Copy, Debug, EnumIter, EnumCount, EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
  /// Migrate VC
  MigrateVoiceConversion,
  /// Migrate TTS
  MigrateTts,
}

#[derive(Parser, Debug)]
#[command(name="migration-tool")]
pub struct Args {
  #[arg(name="action", long="action", help="action to take", required=true)]
  action: String,
}

pub fn parse_cli_args() -> AnyhowResult<ParsedArgs> {
  let args = Args::parse();

  Ok(ParsedArgs {
    action: action_from_str(&args.action)?,
  })
}


fn action_from_str(value: &str) -> AnyhowResult<Action> {
  let action = Action::from_str(value)
      .map_err(|err| {
        let choices = Action::iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();
        anyhow!("parse error: {:?}, provided: \"{}\" choices: {:?}", err, value, choices)
      })?;
  Ok(action)
}
