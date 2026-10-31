//! [`StatsigMaterial`] — the reusable, per-session material an
//! `x-statsig-id` is minted from, plus the pure signature assembly.

use crate::statsig::Statsig;
use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Unix-time reference the embedded `number` counts from (2023-05-01).
const STATSIG_EPOCH: i64 = 1_682_924_400;

/// Salt folded into the signed message (best-known; absent from current bundles).
const STATSIG_SALT: &str = "obfiowerehiring";

/// Trailing marker byte of the assembled payload.
const STATSIG_MARK: u8 = 0x03;

/// The reusable material an `x-statsig-id` is minted from, independent of the
/// endpoint. A signature binds `method`, `path`, and a timestamp — all
/// per-request — around a "genuine hex" fingerprint **derived from the seed**.
/// So the one thing worth storing across requests is the 48-byte **seed**
/// (decoded from a real captured signature): with it we can mint a fresh
/// signature for any endpoint via [`generate_statsig`](Self::generate_statsig),
/// no browser required — see the caveat there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsigMaterial {
  /// The 48-byte seed, base64 (standard alphabet, no padding).
  pub seed_b64: String,
}

impl StatsigMaterial {
  pub fn from_seed_b64(seed_b64: impl Into<String>) -> Self {
    Self { seed_b64: seed_b64.into() }
  }

  /// Mint a signature for `method` + `path` stamped at `generated_at`.
  ///
  /// # Caveat: needs the seed → hex fingerprint
  ///
  /// The signature folds in a "genuine hex" that Grok derives from the seed by
  /// sampling a CSS-animated SVG in-browser, rotating the constants over time.
  /// [`compute_genuine_hex`] — the headless port of that — is currently
  /// unavailable (stale + out of scope to extract), so this returns `None`.
  /// The *assembly* around it ([`assemble`]) is complete and tested, so the
  /// moment a real fingerprint lands, on-the-fly generation works.
  pub fn generate_statsig(&self, method: &str, path: &str, generated_at: DateTime<Utc>) -> Option<Statsig> {
    let seed = BASE64_STANDARD_NO_PAD.decode(self.seed_b64.trim()).ok()?;
    let genuine_hex = compute_genuine_hex(&seed)?;
    Some(assemble(&seed, method, path, generated_at, &genuine_hex))
  }
}

/// Assemble the 70-byte payload from every input (pure, deterministic).
///
/// Layout: `[key][seed^key (48)][number_LE^key (4)][sha256(msg)[..16]^key (16)][mark^key]`,
/// where `msg = METHOD!path!<number-decimal><salt><genuine_hex>` and
/// `number = generated_at - epoch`.
fn assemble(seed: &[u8], method: &str, path: &str, generated_at: DateTime<Utc>, genuine_hex: &str) -> Statsig {
  let method = method.trim().to_ascii_uppercase();
  let path = path.trim();
  let number = generated_at.timestamp().saturating_sub(STATSIG_EPOCH).max(0) as u32;

  let message = format!("{method}!{path}!{number}{STATSIG_SALT}{genuine_hex}");
  let digest = Sha256::digest(message.as_bytes());

  // Any byte works as the XOR mask; derive one deterministically from the seed
  // and timestamp so output is reproducible.
  let key = seed.first().copied().unwrap_or(0) ^ (number as u8);

  let number_bytes = number.to_le_bytes();
  let mut out = Vec::with_capacity(70);
  out.push(key);
  out.extend(seed.iter().map(|byte| byte ^ key));
  out.extend(number_bytes.iter().map(|byte| byte ^ key));
  out.extend(digest[..16].iter().map(|byte| byte ^ key));
  out.push(STATSIG_MARK ^ key);

  Statsig {
    statsig: BASE64_STANDARD_NO_PAD.encode(out),
    generated_at,
    method,
    path: path.to_string(),
  }
}

/// Grok's seed → "genuine hex" fingerprint. Currently unavailable: Grok derives
/// it in-browser by sampling a rotating CSS-animated SVG, and the headless port
/// is stale (extracting the live one is out of scope / blocked). Until this
/// returns a real value, [`StatsigMaterial::generate_statsig`] yields `None`.
/// This is the single missing piece — the assembly around it is done.
fn compute_genuine_hex(_seed: &[u8]) -> Option<String> {
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  const SEED: [u8; 48] = [7u8; 48];
  const NEW_CONVERSATION: &str = "/rest/app-chat/conversations/new";

  #[test]
  fn assemble_produces_a_self_consistent_signature() {
    let generated_at = DateTime::from_timestamp(1_787_535_100, 0).unwrap();
    let hex = "3bab9506b851eb85"; // any fingerprint; the assembly is what's under test

    let signature = assemble(&SEED, "post", NEW_CONVERSATION, generated_at, hex);
    assert_eq!(signature.method, "POST");
    assert_eq!(signature.path, NEW_CONVERSATION);

    // Decode the payload and check every region reconstructs its input.
    let raw = BASE64_STANDARD_NO_PAD.decode(&signature.statsig).unwrap();
    assert_eq!(raw.len(), 70);
    let key = raw[0];
    let unmask = |slice: &[u8]| -> Vec<u8> { slice.iter().map(|b| b ^ key).collect() };

    assert_eq!(unmask(&raw[1..49]), SEED);

    let number_bytes = unmask(&raw[49..53]);
    let number = u32::from_le_bytes([number_bytes[0], number_bytes[1], number_bytes[2], number_bytes[3]]);
    assert_eq!(number as i64, 1_787_535_100 - STATSIG_EPOCH);

    let digest = unmask(&raw[53..69]);
    let expected = &Sha256::digest(
      format!("POST!{NEW_CONVERSATION}!{number}{STATSIG_SALT}{hex}").as_bytes(),
    )[..16];
    assert_eq!(digest, expected);

    assert_eq!(raw[69] ^ key, STATSIG_MARK);
  }

  #[test]
  fn same_seed_different_endpoints_differ() {
    let at = DateTime::from_timestamp(1_787_535_100, 0).unwrap();
    let a = assemble(&SEED, "POST", "/a", at, "beef");
    let b = assemble(&SEED, "POST", "/b", at, "beef");
    assert_ne!(a.statsig, b.statsig, "different paths must sign differently");
  }

  #[test]
  fn generate_statsig_is_none_until_fingerprint_available() {
    // The seed is present, but the seed -> hex fingerprint is not, so no
    // signature can be minted yet.
    let material = StatsigMaterial::from_seed_b64(BASE64_STANDARD_NO_PAD.encode(SEED));
    assert!(material.generate_statsig("POST", NEW_CONVERSATION, Utc::now()).is_none());
  }
}
