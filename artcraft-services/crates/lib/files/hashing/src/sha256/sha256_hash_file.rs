use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use data_encoding::HEXLOWER_PERMISSIVE;

use errors::AnyhowResult;

use crate::sha256::sha256_digest_buffer::sha256_digest_buffer;

pub fn sha256_hash_file<P: AsRef<Path>>(filename: P) -> AnyhowResult<String> {
  let input = File::open(filename)?;
  let reader = BufReader::new(input);
  let digest = sha256_digest_buffer(reader)?;

  let hash = HEXLOWER_PERMISSIVE.encode(digest.as_ref());
  Ok(hash)
}

#[cfg(test)]
pub mod tests {
  use crate::sha256::sha256_hash_file::sha256_hash_file;

  #[test]
  fn test_sha256_hash_file_1() {
    assert_eq!(
      "8d593e85d8c30620b57af0734c010a8cd00083a5a2ffd1ffb91f2c9e85d8535d",
      sha256_hash_file("../../../../test_data/audio/aac/golden_sun_elemental_stars_cyanne.aac")
          .unwrap());
  }

  #[test]
  fn test_sha256_hash_file_2() {
    assert_eq!(
      "452812e5b5269f74b13175e0ef739255ee094f55c9d45a9336ed129cf7c66f8e",
      sha256_hash_file("../../../../test_data/video/webm/laser_pong.webm")
          .unwrap());
  }
}
