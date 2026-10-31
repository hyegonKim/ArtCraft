//! The `statsig.toml` cache: harvested `x-statsig-id` signatures plus every
//! piece we can decode out of them, so a future pure-Rust minter has its
//! substrate ready.
//!
//! # What is (and is not) reusable
//!
//! A signature is only reusable *whole*, and only within its freshness window
//! (its embedded second-precision timestamp). We can decode the seed, key,
//! timestamp, and digest from a captured signature, but **not** the genuine-hex
//! fingerprint — it lives inside the SHA-256 and Grok re-derives it from the
//! seed server-side. Minting a brand-new signature (fresh timestamp) therefore
//! still needs the signer's `seed -> hex` function, which only the live browser
//! has. We cache what we can observe; the browser minting lives in the
//! `grok_consumer_statsig` crate.

use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
use errors::{bail, AnyhowResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

// Payload-layout constants. The signature is minted by a real browser (see
// `grok_consumer_statsig`); we only *decode* a captured one here, so these
// describe the byte layout rather than any (dated, removed) signing algorithm.

/// Seed length in bytes.
const SEED_LEN: usize = 48;

/// Unix-time reference the embedded `number` counts from (2023-05-01).
const STATSIG_EPOCH: i64 = 1_682_924_400;

/// Salt the signer folds into the message (best-known; absent from live bundles).
const STATSIG_SALT: &str = "obfiowerehiring";

/// Trailing marker byte of the assembled payload.
const STATSIG_MARK: u8 = 0x03;

/// Assembled payload length: `key(1) + seed(48) + number(4) + digest(16) + mark(1)`.
const PAYLOAD_LEN: usize = 1 + SEED_LEN + 4 + 16 + 1;

/// The whole `statsig.toml` document.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatsigCacheFile {
  /// The salt the signer folds into the message. Stable until Grok rotates it
  /// (it is currently absent from live bundles, so treat as best-known).
  pub salt: String,

  /// Unix-time reference the embedded `number` counts from.
  pub epoch_unix: i64,

  /// When this file was written (unix seconds).
  pub harvested_at_unix: i64,

  /// One entry per `(method, path)` observed, freshest wins.
  #[serde(default)]
  pub captured: Vec<CapturedStatsig>,
}

/// One harvested signature and everything decodable from it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapturedStatsig {
  /// Uppercase HTTP method the signature was minted for.
  pub method: String,

  /// Request path the signature was minted for.
  pub path: String,

  /// The full opaque `x-statsig-id` header value (reusable while fresh).
  pub statsig_id: String,

  /// When we observed it in the browser (unix seconds).
  pub captured_at_unix: i64,

  // --- decoded pieces (informational + substrate for a future pure minter) ---
  /// The one-byte XOR mask that masks every region after byte 0.
  pub key: u8,

  /// The 48-byte seed the fingerprint is derived from (base64).
  pub seed_b64: String,

  /// `signed_at_unix - epoch_unix`, as embedded in the payload.
  pub number: u32,

  /// Absolute time the browser signed at (`number + epoch`).
  pub signed_at_unix: i64,

  /// The `sha256(method!path!<number><salt><hex>)[..16]` prefix (hex). The hex
  /// fingerprint itself is not recoverable from this.
  pub digest_hex: String,

  /// The trailing marker byte (expected `3`).
  pub mark: u8,
}

impl StatsigCacheFile {
  /// A fresh document stamped at `harvested_at_unix`.
  pub fn new(harvested_at_unix: i64) -> Self {
    Self {
      salt: STATSIG_SALT.to_string(),
      epoch_unix: STATSIG_EPOCH,
      harvested_at_unix,
      captured: Vec::new(),
    }
  }

  /// Insert or replace the entry for a `(method, path)`, keeping whichever
  /// signature was captured most recently.
  pub fn upsert(&mut self, entry: CapturedStatsig) {
    match self.captured.iter_mut().find(|e| e.method == entry.method && e.path == entry.path) {
      Some(existing) if entry.captured_at_unix >= existing.captured_at_unix => *existing = entry,
      Some(_) => {}
      None => self.captured.push(entry),
    }
  }

  pub fn to_toml(&self) -> AnyhowResult<String> {
    Ok(toml::to_string_pretty(self)?)
  }

  pub fn save(&self, path: impl AsRef<Path>) -> AnyhowResult<()> {
    std::fs::write(path, self.to_toml()?)?;
    Ok(())
  }

  pub fn load(path: impl AsRef<Path>) -> AnyhowResult<Self> {
    Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
  }
}

/// Decode a captured `x-statsig-id` into its parts.
///
/// The payload is base64 (standard alphabet, no padding); byte 0 is the key and
/// every later region is XOR-masked by it:
/// `[key][seed^key (48)][number_LE^key (4)][digest[..16]^key (16)][mark^key (1)]`.
pub fn decode_statsig(
  statsig_id: &str,
  method: &str,
  path: &str,
  captured_at_unix: i64,
) -> AnyhowResult<CapturedStatsig> {
  let raw = BASE64_STANDARD_NO_PAD.decode(statsig_id.trim())?;
  if raw.len() != PAYLOAD_LEN {
    bail!("statsig payload is {} bytes, expected {PAYLOAD_LEN}", raw.len());
  }

  let key = raw[0];
  let unmask = |slice: &[u8]| -> Vec<u8> { slice.iter().map(|b| b ^ key).collect() };

  let seed = unmask(&raw[1..1 + SEED_LEN]);
  let number_bytes = unmask(&raw[1 + SEED_LEN..1 + SEED_LEN + 4]);
  let number = u32::from_le_bytes([number_bytes[0], number_bytes[1], number_bytes[2], number_bytes[3]]);
  let digest = unmask(&raw[1 + SEED_LEN + 4..1 + SEED_LEN + 4 + 16]);
  let mark = raw[PAYLOAD_LEN - 1] ^ key;

  Ok(CapturedStatsig {
    method: method.trim().to_ascii_uppercase(),
    path: path.trim().to_string(),
    statsig_id: statsig_id.trim().to_string(),
    captured_at_unix,
    key,
    seed_b64: BASE64_STANDARD_NO_PAD.encode(&seed),
    number,
    signed_at_unix: number as i64 + STATSIG_EPOCH,
    digest_hex: to_hex(&digest),
    mark,
  })
}

fn to_hex(bytes: &[u8]) -> String {
  let mut out = String::with_capacity(bytes.len() * 2);
  for byte in bytes {
    out.push_str(&format!("{byte:02x}"));
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  // A known-good, browser-captured signature for POST /conversations/new
  // (external/requests/.../18_generate_video.txt) — it generated a real video.
  const CAPTURE_18: &str =
    "XbHvyYh7hmNc+8sVAGnrpcuHN/MK+mgN63cmnBljOJaoCKu6zTPVw6C7HVv1wculKCNmYVum+L1h0IjFmtyZD/C53M+ZXg";
  const NEW_CONVERSATION: &str = "/rest/app-chat/conversations/new";

  #[test]
  fn decodes_known_capture_layout() {
    let decoded = decode_statsig(CAPTURE_18, "POST", NEW_CONVERSATION, 1_787_535_100).unwrap();
    assert_eq!(decoded.key, 93);
    assert_eq!(decoded.mark, STATSIG_MARK, "trailing marker should decode to 3");
    assert_eq!(decoded.number, 104_610_686);
    assert_eq!(decoded.signed_at_unix, 104_610_686 + STATSIG_EPOCH);
    assert_eq!(decoded.digest_hex.len(), 32, "16 digest bytes -> 32 hex chars");
    // Seed is 48 bytes -> 64 base64 chars (no padding).
    assert_eq!(BASE64_STANDARD_NO_PAD.decode(&decoded.seed_b64).unwrap().len(), SEED_LEN);
  }

  #[test]
  fn rejects_wrong_length() {
    assert!(decode_statsig("YWJj", "POST", "/x", 0).is_err());
  }

  #[test]
  fn upsert_keeps_the_freshest_per_endpoint() {
    let mut file = StatsigCacheFile::new(1_000);
    let older = decode_statsig(CAPTURE_18, "POST", NEW_CONVERSATION, 1_000).unwrap();
    let mut newer = older.clone();
    newer.captured_at_unix = 2_000;
    file.upsert(older);
    file.upsert(newer);
    assert_eq!(file.captured.len(), 1);
    assert_eq!(file.captured[0].captured_at_unix, 2_000);
  }

  #[test]
  fn toml_round_trips() {
    let mut file = StatsigCacheFile::new(1_787_535_100);
    file.upsert(decode_statsig(CAPTURE_18, "POST", NEW_CONVERSATION, 1_787_535_100).unwrap());
    let text = file.to_toml().unwrap();
    let parsed: StatsigCacheFile = toml::from_str(&text).unwrap();
    assert_eq!(parsed.captured.len(), 1);
    assert_eq!(parsed.captured[0].statsig_id, CAPTURE_18);
  }

  // Tool 1 (offline): run the decode + serialize pipeline on a real capture and
  // print the signature and every cacheable piece. This is the same code the
  // `harvest_statsig` binary writes to statsig.toml; the live capture happens in
  // that binary (a native event loop cannot cleanly run inside `cargo test`).
  #[test]
  fn print_statsig_and_cacheable_pieces() {
    let mut file = StatsigCacheFile::new(1_787_535_100);
    file.upsert(decode_statsig(CAPTURE_18, "POST", NEW_CONVERSATION, 1_787_535_100).unwrap());

    let entry = &file.captured[0];
    println!("\n===== harvested statsig =====");
    println!("x-statsig-id : {}", entry.statsig_id);
    println!("method/path  : {} {}", entry.method, entry.path);
    println!("--- decoded (cacheable) pieces ---");
    println!("key          : {}", entry.key);
    println!("seed (b64)   : {}", entry.seed_b64);
    println!("number       : {}", entry.number);
    println!("signed_at    : {} (unix)", entry.signed_at_unix);
    println!("digest[..16] : {}", entry.digest_hex);
    println!("mark         : {}", entry.mark);
    println!("--- statsig.toml ---\n{}", file.to_toml().unwrap());
  }
}
