use std::str::FromStr;

use clap::Parser;
use strum::{EnumCount, EnumString, IntoEnumIterator};
use strum::EnumIter;

use errors::{anyhow, AnyhowResult, bail};

pub struct ParsedArgs {
  pub mysql_environment: Environment,
  pub elasticsearch_environment: Environment,
  pub action: Action,
}

#[derive(Clone, Copy, Debug)]
pub enum Environment {
  Development,
  Production,
}

#[derive(Clone, Copy, Debug, EnumIter, EnumCount, EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
  ReindexTts,
  SearchTts,

  /// Reindex model_weights into ElasticSearch
  ReindexModelWeights,

  /// Search the model weights
  SearchModelWeights,

  /// Evaluate the search algorithm
  EvaluateModelWeights,

  /// Reindex media_files into ElasticSearch
  /// For now, this is just media files that are relevant to the engine.
  ReindexMediaFiles,
  SearchMediaFiles,
}

#[derive(Parser, Debug)]
#[command(name="elasticsearch-cli")]
pub struct Args {
  #[arg(name="action", long="action", help="action to take", required=true)]
  action: Action,

  #[arg(name="mysql", long="mysql", help="production or development")]
  mysql: Option<String>,

  #[arg(name="elasticsearch", long="elasticsearch", help="production or development")]
  elasticsearch: Option<String>,
}

pub fn parse_cli_args() -> AnyhowResult<ParsedArgs> {
  let args = Args::parse();

  Ok(ParsedArgs {
    mysql_environment: to_environment(args.mysql.as_deref())?,
    elasticsearch_environment: to_environment(args.elasticsearch.as_deref())?,
    action: args.action,
  })
}

fn to_environment(environment: Option<&str>) -> AnyhowResult<Environment> {
  Ok(match environment {
    None => Environment::Development,
    Some("dev") | Some("development") => Environment::Development,
    Some("prod") | Some("production") => Environment::Production,
    _ => bail!("invalid environment: {:?}", environment),
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
