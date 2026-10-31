use std::io::Cursor;

use base64;
use base64::alphabet::{Alphabet, URL_SAFE};
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine;
use magic_crypt::{MagicCrypt256, MagicCryptTrait};
use magic_crypt::generic_array::typenum::U256;
use magic_crypt::new_magic_crypt;

use errors::AnyhowResult;

/// This tool exists so that we don't leak our database IDs!
///
/// If competitors (or VCs) see our IDs, they'll know how big our database
/// is (since IDs are auto_increment), and they can use that information to
/// estimate our success, unit economics, etc.
///
/// We can still use our IDs in the frontend APIs for sorting and pagination
/// if we encrypt them before delivering them to the client. This prevents
/// us from leaking "business secret information".
///
/// To some degree this may be mild paranoia, but I've always seen this
/// practice taken in industry (unicorn+ companies) and in the startup world.
///
#[derive(Clone)]
pub struct SortKeyCrypto {
  crypt: MagicCrypt256,
  alphabet: Alphabet,
  base64_config: GeneralPurposeConfig,
}

// TODO: A protobuf would be more compact than JSON!
/// This gets encrypted and sent to the frontend as an opaque handle.
#[derive(Serialize, Deserialize)]
pub struct SortId {
  // NB(bt): Entropy is causing React to panic on re-renders and endlessly
  // call the endpoint. We may need to make these stable. It's also possible
  // additional entropy is simply a waste.
  //pub entropy: u32,
  pub column_id: u64,
}

impl SortKeyCrypto {
  pub fn new(secret: &str) -> Self {
    let base64_config = GeneralPurposeConfig::new()
        .with_encode_padding(false)
        .with_decode_allow_trailing_bits(true)
        .with_decode_padding_mode(DecodePaddingMode::Indifferent);
    Self {
      crypt: new_magic_crypt!(secret, 256),
      alphabet: URL_SAFE.clone(),
      base64_config,
    }
  }

  pub fn encrypt_id(&self, id: u64) -> AnyhowResult<String> {
    let mut rng = rand::thread_rng();

    let payload = SortId {
      //entropy: rng.next_u32(),
      column_id: id,
    };

    let payload = serde_json::to_string(&payload)?;

    let mut reader = Cursor::new(payload);
    let mut writer = Vec::new();

    let engine = GeneralPurpose::new(&self.alphabet, self.base64_config);

    self.crypt.encrypt_reader_to_writer2::<U256>(&mut reader, &mut writer)?;
    let encoded = engine.encode(&writer);

    Ok(encoded)
  }

  pub fn decrypt_id(&self, base_64_payload: &str) -> AnyhowResult<u64> {
    //let payload = self.crypt.decrypt_base64_to_string(base_64_payload)?;

    let engine = GeneralPurpose::new(&self.alphabet, self.base64_config);

    let decoded_bytes = engine.decode(base_64_payload)?;
    let decrypted_bytes = self.crypt.decrypt_bytes_to_bytes(&decoded_bytes)?;

    let payload = String::from_utf8(decrypted_bytes)?;
    let payload = serde_json::from_str::<SortId>(&payload)?;

    Ok(payload.column_id)
  }
}

#[cfg(test)]
mod tests {
  use crate::util::encrypted_sort_id::SortKeyCrypto;

//  #[test]
//  fn encrypt_entropy_means_no_duplicate_values() {
//    let sorter = SortKeyCrypto::new("secret");
//
//    let mut encrypted_tokens = HashSet::new();
//
//    for i in 0 .. 1000 {
//      let encrypted = sorter.encrypt_id(1234).unwrap();
//      encrypted_tokens.insert(encrypted);
//    }
//
//    assert_eq!(encrypted_tokens.len(), 1000);
//  }

  #[test]
  fn decode_encrypted() {
    let encrypted = "n4CjiUNwL6zEM61mJxvLCxD3wiZa8d6_O70ICRVzNUg";
    let sorter = SortKeyCrypto::new("secret");
    let decrypted = sorter.decrypt_id(&encrypted).unwrap();
    assert_eq!(decrypted, 1234);

    let encrypted = "NL5wfw85lJsVtQ4QEPmoe-TWNDNnZU7TZgSqMBXCGAA";
    let sorter = SortKeyCrypto::new("different secret");
    let decrypted = sorter.decrypt_id(&encrypted).unwrap();
    assert_eq!(decrypted, 54321);
  }

  #[test]
  fn encrypt_roundtrip() {
    let sorter = SortKeyCrypto::new("secret");
    let encrypted = sorter.encrypt_id(1234).unwrap();
    let decrypted = sorter.decrypt_id(&encrypted).unwrap();
    assert_eq!(decrypted, 1234);
  }

  #[test]
  fn encrypt_lots_roundtrip_low_numbers() {
    let sorter = SortKeyCrypto::new("secret");

    for i in 0 .. 1000 {
      let encrypted = sorter.encrypt_id(i).unwrap();
      let decrypted = sorter.decrypt_id(&encrypted).unwrap();
      assert_eq!(decrypted, i);
    }
  }

  #[test]
  fn encrypt_lots_roundtrip_high_numbers() {
    let sorter = SortKeyCrypto::new("secret");

    for i in 100000000 .. 100005000 {
      let encrypted = sorter.encrypt_id(i).unwrap();
      let decrypted = sorter.decrypt_id(&encrypted).unwrap();
      assert_eq!(decrypted, i);
    }
  }
}
