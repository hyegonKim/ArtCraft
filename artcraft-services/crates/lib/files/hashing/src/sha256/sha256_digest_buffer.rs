use std::io::Read;

use ring::digest::{Context, Digest, SHA256};

use errors::AnyhowResult;

#[inline]
pub fn sha256_digest_buffer<R: Read>(mut reader: R) -> AnyhowResult<Digest> {
  let mut context = Context::new(&SHA256);
  let mut buffer = [0; 1024];

  loop {
    let count = reader.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    context.update(&buffer[..count]);
  }

  Ok(context.finish())
}
