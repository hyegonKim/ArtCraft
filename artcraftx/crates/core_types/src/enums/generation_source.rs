//! This is an important enum.
//!
//! It identifies where a generation comes from: the service (and auth
//! mechanism) behind a stored credential, and the provider recorded for
//! tasks in the local tasks database.
//!
//! The string values are stored in user-editable credential TOML files AND
//! in the sqlite tasks database. NEVER change existing values; only add
//! new ones. Keep the max length to 32 characters.

use std::collections::BTreeSet;
use std::fmt;

use crate::enums::enum_error::EnumError;
#[cfg(test)]
use strum::EnumCount;
#[cfg(test)]
use strum::EnumIter;

/// Whether a source authenticates with browser cookies or an API key.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum CredentialKind {
  #[serde(rename = "cookies")]
  Cookies,
  #[serde(rename = "api_key")]
  ApiKey,
}

/// Where a generation comes from: a service plus its auth mechanism.
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum GenerationSource {
  // ── ArtCraft ──
  /// Production ArtCraft API (username + password login; stores the
  /// resulting session cookie).
  #[serde(rename = "artcraft")]
  Artcraft,
  /// Local dev ArtCraft API.
  #[serde(rename = "artcraft_local")]
  ArtcraftLocal,
  /// ArtCraft website cookies captured from a webview.
  #[serde(rename = "artcraft_cookies")]
  ArtcraftCookies,
  /// ArtCraft API key.
  #[serde(rename = "artcraft_api")]
  ArtcraftApi,

  // ── Fal ──
  #[serde(rename = "fal")]
  Fal,
  #[serde(rename = "fal_api")]
  FalApi,

  // ── Grok / xAI ──
  #[serde(rename = "grok")]
  Grok,
  // `xai_cookies` is the legacy name for this service; kept as a serde alias so
  // existing credential files still load.
  #[serde(rename = "grok_cookies", alias = "xai_cookies")]
  GrokCookies,
  #[serde(rename = "xai_api")]
  XAiApi,

  // ── Midjourney ──
  #[serde(rename = "midjourney")]
  Midjourney,
  #[serde(rename = "midjourney_cookies")]
  MidjourneyCookies,

  // ── OpenAI ──
  #[serde(rename = "openai_api")]
  OpenAiApi,

  // ── World Labs ──
  #[serde(rename = "world_labs")]
  WorldLabs,
  #[serde(rename = "worldlabs_cookies")]
  WorldLabsCookies,

  // ── Higgsfield ──
  /// The generation provider recorded on tasks run with a Higgsfield account.
  #[serde(rename = "higgsfield")]
  Higgsfield,
  /// Higgsfield website cookies captured from the login webview.
  #[serde(rename = "higgsfield_cookies")]
  HiggsfieldCookies,
  // ── Other website (cookie) integrations ──
  #[serde(rename = "magnific_cookies")]
  MagnificCookies,
  #[serde(rename = "openart_cookies")]
  OpenArtCookies,
  #[serde(rename = "runway_cookies")]
  RunwayCookies,

  // ── Other API key integrations ──
  #[serde(rename = "replicate_api")]
  ReplicateApi,
}

impl GenerationSource {
  /// The auth mechanism this source uses.
  pub fn kind(&self) -> CredentialKind {
    match self {
      Self::Artcraft
      | Self::ArtcraftLocal
      | Self::ArtcraftCookies
      | Self::Grok
      | Self::GrokCookies
      | Self::Midjourney
      | Self::MidjourneyCookies
      | Self::WorldLabs
      | Self::WorldLabsCookies
      | Self::Higgsfield
      | Self::HiggsfieldCookies
      | Self::MagnificCookies
      | Self::OpenArtCookies
      | Self::RunwayCookies => CredentialKind::Cookies,
      Self::ArtcraftApi
      | Self::Fal
      | Self::FalApi
      | Self::OpenAiApi
      | Self::ReplicateApi
      | Self::XAiApi => CredentialKind::ApiKey,
    }
  }

  pub fn to_str(&self) -> &'static str {
    match self {
      Self::Artcraft => "artcraft",
      Self::ArtcraftLocal => "artcraft_local",
      Self::ArtcraftCookies => "artcraft_cookies",
      Self::ArtcraftApi => "artcraft_api",
      Self::Fal => "fal",
      Self::FalApi => "fal_api",
      Self::Grok => "grok",
      Self::GrokCookies => "grok_cookies",
      Self::XAiApi => "xai_api",
      Self::Midjourney => "midjourney",
      Self::MidjourneyCookies => "midjourney_cookies",
      Self::OpenAiApi => "openai_api",
      Self::WorldLabs => "world_labs",
      Self::WorldLabsCookies => "worldlabs_cookies",
      Self::Higgsfield => "higgsfield",
      Self::HiggsfieldCookies => "higgsfield_cookies",
      Self::MagnificCookies => "magnific_cookies",
      Self::OpenArtCookies => "openart_cookies",
      Self::RunwayCookies => "runway_cookies",
      Self::ReplicateApi => "replicate_api",
    }
  }

  pub fn from_str(value: &str) -> Result<Self, EnumError> {
    match value {
      "artcraft" => Ok(Self::Artcraft),
      "artcraft_local" => Ok(Self::ArtcraftLocal),
      "artcraft_cookies" => Ok(Self::ArtcraftCookies),
      "artcraft_api" => Ok(Self::ArtcraftApi),
      "fal" => Ok(Self::Fal),
      "fal_api" => Ok(Self::FalApi),
      "grok" => Ok(Self::Grok),
      "grok_cookies" | "xai_cookies" => Ok(Self::GrokCookies),
      "xai_api" => Ok(Self::XAiApi),
      "midjourney" => Ok(Self::Midjourney),
      "midjourney_cookies" => Ok(Self::MidjourneyCookies),
      "openai_api" => Ok(Self::OpenAiApi),
      "world_labs" => Ok(Self::WorldLabs),
      "worldlabs_cookies" => Ok(Self::WorldLabsCookies),
      "higgsfield" => Ok(Self::Higgsfield),
      "higgsfield_cookies" => Ok(Self::HiggsfieldCookies),
      "magnific_cookies" => Ok(Self::MagnificCookies),
      "openart_cookies" => Ok(Self::OpenArtCookies),
      "runway_cookies" => Ok(Self::RunwayCookies),
      "replicate_api" => Ok(Self::ReplicateApi),
      _ => Err(EnumError::CouldNotConvertFromString(value.to_string())),
    }
  }

  pub fn all_variants() -> BTreeSet<Self> {
    // NB: BTreeSet is sorted
    BTreeSet::from([
      Self::Artcraft,
      Self::ArtcraftLocal,
      Self::ArtcraftCookies,
      Self::ArtcraftApi,
      Self::Fal,
      Self::FalApi,
      Self::Grok,
      Self::GrokCookies,
      Self::XAiApi,
      Self::Midjourney,
      Self::MidjourneyCookies,
      Self::OpenAiApi,
      Self::WorldLabs,
      Self::WorldLabsCookies,
      Self::Higgsfield,
      Self::HiggsfieldCookies,
      Self::MagnificCookies,
      Self::OpenArtCookies,
      Self::RunwayCookies,
      Self::ReplicateApi,
    ])
  }
}

impl fmt::Display for GenerationSource {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.to_str())
  }
}

impl fmt::Debug for GenerationSource {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.to_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod explicit_checks {
    use super::*;

    #[test]
    fn serde_round_trip_matches_to_str() {
      for source in GenerationSource::all_variants() {
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, format!("\"{}\"", source.to_str()));
        let back: GenerationSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, source);
      }
    }

    #[test]
    fn from_str_err() {
      let result = GenerationSource::from_str("asdf");
      assert_eq!(
        result,
        Err(EnumError::CouldNotConvertFromString("asdf".to_string()))
      );
    }

    #[test]
    fn all_variants_count() {
      assert_eq!(GenerationSource::all_variants().len(), 20);
    }

    #[test]
    fn kind_matches_variant_family() {
      assert_eq!(GenerationSource::Artcraft.kind(), CredentialKind::Cookies);
      assert_eq!(GenerationSource::ArtcraftLocal.kind(), CredentialKind::Cookies);
      assert_eq!(GenerationSource::RunwayCookies.kind(), CredentialKind::Cookies);
      assert_eq!(GenerationSource::Fal.kind(), CredentialKind::ApiKey);
      assert_eq!(GenerationSource::FalApi.kind(), CredentialKind::ApiKey);
    }
  }

  mod mechanical_checks {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn variant_length() {
      assert_eq!(GenerationSource::all_variants().len(), GenerationSource::iter().len());
    }

    #[test]
    fn round_trip() {
      for variant in GenerationSource::all_variants() {
        assert_eq!(variant, GenerationSource::from_str(variant.to_str()).unwrap());
        assert_eq!(variant, GenerationSource::from_str(&format!("{}", variant)).unwrap());
        assert_eq!(variant, GenerationSource::from_str(&format!("{:?}", variant)).unwrap());
      }
    }

    /// Stored in VARCHAR-ish columns and TOML files. Every value must fit.
    #[test]
    fn max_serialization_length() {
      const MAX_LENGTH: usize = 32;
      for variant in GenerationSource::all_variants() {
        let serialized = variant.to_str();
        assert!(!serialized.is_empty());
        assert!(serialized.len() <= MAX_LENGTH);
      }
    }
  }
}
