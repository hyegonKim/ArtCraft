use chrono::NaiveDate;
use clap::Parser;
use serde_derive::Deserialize;

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

use crate::args::remaining_args;

#[derive(Clone, Debug)]
pub struct SubArgs {
  // Token to search for
  pub model_token: Option<ModelWeightToken>,

  /// Starting date to calculate for
  pub start_date: Option<NaiveDate>,

  /// Ending date to calculate for
  pub end_date: Option<NaiveDate>,
}

#[derive(Parser, Debug, Deserialize)]
#[command(name="calculate-model-weights-usages")]
#[serde(rename_all = "snake_case")]
struct SubArgsInternal {
  #[arg(name="model-token", long="model-token", help="scope to a model", required=false)]
  model_token: Option<String>,

  #[arg(name="start-date", long="start-date", help="the starting date", required=false)]
  start_date: Option<String>,

  #[arg(name="end-date", long="end-date", help="the ending date", required=false)]
  end_date: Option<String>,
}


pub fn parse_cli_sub_args() -> AnyhowResult<SubArgs> {
  let args = SubArgsInternal::parse_from(remaining_args());

  Ok(SubArgs {
    model_token: args.model_token
        .map(|token| ModelWeightToken::new_from_str(&token)),
    start_date: args.start_date
        .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d"))
        .transpose()?,
    end_date: args.end_date
        .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d"))
        .transpose()?,
  })
}
