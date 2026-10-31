use std::io::BufReader;

use data_encoding::HEXLOWER_PERMISSIVE;

use errors::AnyhowResult;

use crate::sha256::sha256_digest_buffer::sha256_digest_buffer;

pub fn sha256_hash_bytes(bytes: &[u8]) -> AnyhowResult<String> {
  let reader = BufReader::new(bytes);
  let digest = sha256_digest_buffer(reader)?;
  let hash = HEXLOWER_PERMISSIVE.encode(digest.as_ref());
  Ok(hash)
}

#[cfg(test)]
mod tests {
  use crate::sha256::sha256_hash_bytes::sha256_hash_bytes;

  #[test]
  fn test_sha256_hash_bytes() {
    assert_eq!(sha256_hash_bytes(&[]).unwrap(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256_hash_bytes(&[0, 0, 0, 0]).unwrap(), "df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119");
    assert_eq!(sha256_hash_bytes(&[0, 255, 0, 255]).unwrap(), "7a7bf454c5f3cb1b9d9a20f81417f98d976fe3b3dd52c1b9968f02e89e7e8a2f");
    assert_eq!(sha256_hash_bytes("testing".as_bytes()).unwrap(), "cf80cd8aed482d5d1527d7dc72fceff84e6326592848447d2dc0b0e87dfc9a90");
    assert_eq!(sha256_hash_bytes("hash this".as_bytes()).unwrap(), "19467788bc0cf11790a075ea718452cecf0e79db59d1964670475e5fe2e4a611");
  }
}
