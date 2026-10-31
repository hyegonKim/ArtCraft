use log::info;

use easyenv::init_all_with_default_logging;
use errors::AnyhowResult;

fn main() -> AnyhowResult<()> {
    init_all_with_default_logging(None);

    info!("TODO...");

    Ok(())
}