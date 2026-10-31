use log::info;
use sqlx::{MySql, Pool};
use sqlx::mysql::MySqlPoolOptions;

use easyenv::init_all_with_default_logging;
use errors::AnyhowResult;

use crate::cli_args::Action;
use crate::operations::delete_all_anonymous_user_images::delete_all_anonymous_user_images::delete_all_anonymous_user_images;
use crate::operations::delete_user_files::delete_user_files::delete_user_files;
use crate::operations::migrate_media_files_enum_values::migrate_media_files_enum_values::migrate_media_files_enum_values;

mod cli_args;
mod operations;
mod util;

/*

Users that have had files deleted:
- cheesetherat
- endtimes
- expandinggirl
- markiwrench
- regista2
- regista77

TODO:
- juanfra3567
- ceistin
- elfabro17
- sodoka
- fluffernater

*/

#[tokio::main]
async fn main() -> AnyhowResult<()> {
  println!("db-cleanup: hard or soft delete database records");

  init_all_with_default_logging(None);

  // NB: This secrets file differs from the rest because we might want to delete from production.
  // (Hopefully this isn't getting out of hand at this point.)
  easyenv::from_filename(".env-db-cleanup-secrets")?;

  let args = cli_args::parse_cli_args()?;

  let mysql = get_mysql("MYSQL_PRODUCTION_URL").await?;

  match args.action {
    Action::DeleteAllAnonymousUserImages => {
      delete_all_anonymous_user_images(&args, &mysql).await?;
    }
    Action::DeleteUserFiles => {
      delete_user_files(&args, &mysql).await?;
    }
    Action::MigrateMediaFilesEnumValues => {
      migrate_media_files_enum_values(&args, &mysql).await?;
    }
  }

  Ok(())
}

async fn get_mysql(env_var_name: &str) -> errors::AnyhowResult<Pool<MySql>> {
  info!("Connecting to MySQL {env_var_name}...");

  let pool = MySqlPoolOptions::new()
      .max_connections(easyenv::get_env_num("MYSQL_MAX_CONNECTIONS", 3)?)
      .connect(&easyenv::get_env_string_required(env_var_name)?)
      .await?;

  Ok(pool)
}
