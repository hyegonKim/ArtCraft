//use bytes::BytesMut;
use actix_multipart::Field;
use actix_web::web::BytesMut;
use anyhow::anyhow;
use futures::{StreamExt, TryStreamExt};
use log::warn;

const MIN_BYTES : usize = 10;
//const MAX_BYTES : usize = 1024 * 1024 * 20; // 20 Mb
const MAX_BYTES : usize = 1024 * 1024 * 1024 * 10; // 10 GiB

/// Read any field from a multipart form as bytes, but do some error checking.
pub async fn checked_read_multipart_bytes(field: &mut Field) -> anyhow::Result<Option<BytesMut>> {
  let bytes = read_multipart_field_bytes(field).await?;

  if bytes.is_empty() {
    // This field is empty
    return Ok(None);
  }

  if bytes.len() < MIN_BYTES {
    warn!("Bytes of upload too few: {}", bytes.len());
    return Err(anyhow!("Too few bytes"));
  }

  if bytes.len() > MAX_BYTES {
    warn!("Bytes of upload too much: {}", bytes.len());
    return Err(anyhow!("Too many bytes"));
  }

  Ok(Some(bytes))
}

/// Read any field from a multipart form as bytes.
pub async fn read_multipart_field_bytes(field: &mut Field) -> anyhow::Result<BytesMut> {
  let mut bytes = BytesMut::new();
  while let Some(chunk) = field.next().await {
    let chunk_data = chunk.map_err(|err| anyhow!("Error: {:?}", err))?;
    bytes.extend_from_slice(&chunk_data);
  }
  Ok(bytes)
}

/// This reads inputs, text boxes, etc. because form-multipart is hard.
pub async fn read_multipart_field_as_text(field: &mut Field) -> anyhow::Result<Option<String>> {
  let bytes = read_multipart_field_bytes(field).await?;
  let value = String::from_utf8(bytes.to_vec())?;

  match value.as_str() {
    ""
      | "none"
      | "undefined" // Javascript can send the string "undefined" for undefined payload values.
    => Ok(None),
    _ => Ok(Some(value)),
  }
}

/// Read a html form boolean (eg. input type=checkbox); assumes default is false.
pub async fn read_multipart_field_as_boolean(field: &mut Field) -> anyhow::Result<bool> {
  let result = read_multipart_field_as_text(field).await?;
  Ok(match result.as_ref().map(String::as_str) {
    Some("true") => true,
    Some("TRUE") => true,
    Some("false") => false,
    Some("FALSE") => false,
    None => false,
    _ => true,
  })
}
