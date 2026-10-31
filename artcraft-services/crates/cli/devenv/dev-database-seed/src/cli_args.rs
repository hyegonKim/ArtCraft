use clap::Parser;

use errors::AnyhowResult;

#[derive(Parser, Debug)]
#[command(name="dev-database-seed")]
pub struct CliArgs {
  #[arg(name="seed_cloud_bucket", long="bucket", help="Seed the cloud bucket with files", required=false, default_value_t=false)]
  pub seed_cloud_bucket: bool,

  #[arg(name="seed_elasticsearch", long="elasticsearch", help="Seed the local elasticsearch", required=false, default_value_t=false)]
  pub seed_elasticsearch: bool,
}

pub fn parse_cli_args() -> AnyhowResult<CliArgs> {
  Ok(CliArgs::parse())
}
