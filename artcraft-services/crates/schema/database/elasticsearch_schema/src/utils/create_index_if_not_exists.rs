use elasticsearch::Elasticsearch;
use elasticsearch::http::StatusCode;
use elasticsearch::indices::{IndicesCreateParts, IndicesDeleteParts, IndicesExistsParts};
use log::{error, info};
use serde_json::Value;

use errors::{AnyhowResult, bail};

pub struct CreateIndexArgs <'a> {
  pub client: &'a Elasticsearch,
  pub index_name: &'a str,
  pub index_definition: &'a str,
  pub delete_existing: bool,
}

// NB: Adapted from elasticsearch crate examples source
pub async fn create_index_if_not_exists<'a>(args: CreateIndexArgs<'a>) -> AnyhowResult<()> {
  info!("Checking if Elasticsearch index exists: {:?}", args.index_name);

  let exists = args.client
      .indices()
      .exists(IndicesExistsParts::Index(&[args.index_name]))
      .send()
      .await?;

  if exists.status_code().is_success() && args.delete_existing {
    info!("Deleting Elasticsearch index: {:?}", args.index_name);

    let delete = args.client
        .indices()
        .delete(IndicesDeleteParts::Index(&[args.index_name]))
        .send()
        .await?;

    if !delete.status_code().is_success() {
      error!("Problem deleting index: {}", delete.text().await?);
    }
  }

  if exists.status_code() == StatusCode::NOT_FOUND || args.delete_existing {
    // NB(bt): This is weirdly tricky.
    // Passing a value to `.body()` with serde_json's json!() literal macro works.
    // However, Elasticsearch server fails if the argument to `.body()` is a string.
    // Furthermore, Elasticsearch server fails if serde_json::from_str() is called
    // without a bound type! We need to bind it to the `Value` type explicitly.
    let json_value : Value = serde_json::from_str(args.index_definition)?;

    let response = args.client
        .indices()
        .create(IndicesCreateParts::Index(args.index_name))
        .body(json_value)
        .send()
        .await?;

    if !response.status_code().is_success() {
      bail!("Error while creating index");
    }
  }

  Ok(())
}
