/// Turn "not found" errors into optional results.
pub fn transform_optional_result<T>(
  result: Result<T, sqlx::Error>
) -> Result<Option<T>, sqlx::Error> {
  match result {
    Ok(record) => Ok(Some(record)),
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(None),
      _ => Err(err)
    }
  }
}
