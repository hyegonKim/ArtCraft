use std::path::PathBuf;

use clap::Parser;

use errors::AnyhowResult;

#[derive(Parser, Debug)]
#[command(name="dev-upload-media-file")]
pub struct RawArgs {
  #[arg(name="file", short='f', long="file", help="File to upload", required=true)]
  pub file_path: String,
}

pub struct CliArgs {
  pub file_path: PathBuf,
}

pub fn parse_cli_args() -> AnyhowResult<CliArgs> {
  let args = RawArgs::parse();

  Ok(CliArgs {
    file_path: PathBuf::from(args.file_path),
  })
}
