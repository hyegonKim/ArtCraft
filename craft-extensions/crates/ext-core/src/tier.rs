//! Trust tiers and the policy that decides which are accepted.
//!
//! Invariants: the tier is assigned by the host from where a package came, never read from the package.
//! The third-party tier is refused unless the `third-party` cargo feature is on, and no release build
//! turns it on until the gates in docs/THREAT-MODEL.md have passed. Only the first-party tier may use
//! first-party id prefixes, so a file or an outside publisher cannot impersonate or replace one.

use crate::id::ExtensionId;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Built and released by the Craft organisation.
    FirstParty,
    /// A file the person added themselves.
    FromFile,
    /// An outside publisher reviewed into the catalogue. Closed in release builds.
    ReviewedThirdParty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TierError {
    ThirdPartyClosed,
    ReservedNamespace,
}

impl fmt::Display for TierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TierError::ThirdPartyClosed => "extensions from outside publishers cannot be installed yet",
            TierError::ReservedNamespace => "the package uses a name that only ArtCraft's own extensions may use",
        })
    }
}

impl std::error::Error for TierError {}

/// What this build accepts. Built only through [`Policy::standard`], so a caller cannot opt in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    _private: (),
}

impl Policy {
    pub fn standard() -> Self {
        Self { _private: () }
    }

    /// Whether packages of this tier may be installed by this build.
    pub fn admit(&self, tier: Tier) -> Result<(), TierError> {
        match tier {
            Tier::FirstParty | Tier::FromFile => Ok(()),
            Tier::ReviewedThirdParty if cfg!(feature = "third-party") => Ok(()),
            Tier::ReviewedThirdParty => Err(TierError::ThirdPartyClosed),
        }
    }

    /// Whether this tier may use this id.
    pub fn check_id(&self, tier: Tier, id: &ExtensionId) -> Result<(), TierError> {
        if tier != Tier::FirstParty && id.in_first_party_namespace() {
            return Err(TierError::ReservedNamespace);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> ExtensionId {
        ExtensionId::new(s).unwrap()
    }

    #[test]
    fn first_party_and_files_are_accepted() {
        let p = Policy::standard();
        assert_eq!(p.admit(Tier::FirstParty), Ok(()));
        assert_eq!(p.admit(Tier::FromFile), Ok(()));
    }

    #[cfg(not(feature = "third-party"))]
    #[test]
    fn third_party_is_closed_in_this_build() {
        assert_eq!(Policy::standard().admit(Tier::ReviewedThirdParty), Err(TierError::ThirdPartyClosed));
    }

    #[cfg(feature = "third-party")]
    #[test]
    fn third_party_is_accepted_only_when_the_feature_is_on() {
        assert_eq!(Policy::standard().admit(Tier::ReviewedThirdParty), Ok(()));
    }

    #[test]
    fn only_first_party_may_use_first_party_names() {
        let p = Policy::standard();
        let reserved = id("org.storytold.dark-ocean");
        assert_eq!(p.check_id(Tier::FirstParty, &reserved), Ok(()));
        assert_eq!(p.check_id(Tier::FromFile, &reserved), Err(TierError::ReservedNamespace));
        assert_eq!(p.check_id(Tier::ReviewedThirdParty, &reserved), Err(TierError::ReservedNamespace));
        let ordinary = id("org.example.dark-ocean");
        for tier in [Tier::FirstParty, Tier::FromFile, Tier::ReviewedThirdParty] {
            assert_eq!(p.check_id(tier, &ordinary), Ok(()));
        }
    }

    #[test]
    fn tiers_serialize_as_plain_words() {
        assert_eq!(serde_json::to_string(&Tier::FirstParty).unwrap(), "\"first-party\"");
        assert_eq!(serde_json::from_str::<Tier>("\"from-file\"").unwrap(), Tier::FromFile);
        assert!(serde_json::from_str::<Tier>("\"trusted\"").is_err());
    }

    #[test]
    fn messages_are_plain_language() {
        for e in [TierError::ThirdPartyClosed, TierError::ReservedNamespace] {
            let text = e.to_string().to_lowercase();
            assert!(!text.contains("namespace") && !text.contains("tier") && !text.contains("feature"), "{text}");
        }
    }
}
