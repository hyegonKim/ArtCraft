//! The catalogue index: what can be installed, and what must be switched off.
//!
//! Invariants: read with a size cap; entries and revocations are bounded; every entry is checked as
//! hard as a manifest; an index that has expired, or is older than one already seen, is refused (the
//! clock and the last seen version are passed in, this crate has neither). Signatures over the index are
//! a later gate; until then only first-party entries are admitted to a release build.

use crate::id::{AppId, ExtensionId};
use crate::manifest::{Kind, MAX_APPS, is_sha256_hex, valid_https_url};
use crate::text::{Lines, validate_text};
use crate::tier::{Policy, Tier};
use crate::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const MAX_INDEX_BYTES: usize = 1024 * 1024;
pub const MAX_ENTRIES: usize = 2000;
pub const MAX_REVOCATIONS: usize = 2000;
pub const SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: ExtensionId,
    pub version: Version,
    pub kind: Kind,
    pub name: String,
    pub tier: Tier,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub apps: Vec<AppId>,
}

/// An extension (or every version of it below `below`) that must be turned off wherever it is installed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revocation {
    pub id: ExtensionId,
    /// Revoke versions strictly below this one. Absent means every version.
    #[serde(default)]
    pub below: Option<Version>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
    /// Grows with every published index. Used to refuse a replayed older one.
    pub version: u64,
    /// Unix seconds after which the index must not be trusted.
    pub expires_at: u64,
    pub entries: Vec<IndexEntry>,
    #[serde(default)]
    pub revoked: Vec<Revocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexError {
    TooLarge,
    NotReadable,
    UnsupportedSchema(u32),
    Expired,
    Older,
    TooManyEntries,
    TooManyRevocations,
    BadEntry,
    DuplicateEntry,
    BadRevocation,
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IndexError::TooLarge => "the catalogue is too large",
            IndexError::NotReadable => "the catalogue cannot be read",
            IndexError::UnsupportedSchema(_) => "the catalogue is in a newer format than this app knows",
            IndexError::Expired => "the catalogue is out of date",
            IndexError::Older => "the catalogue is older than one already seen",
            IndexError::TooManyEntries => "the catalogue lists too many items",
            IndexError::TooManyRevocations => "the catalogue lists too many withdrawn items",
            IndexError::BadEntry => "the catalogue has an item that is not valid",
            IndexError::DuplicateEntry => "the catalogue lists the same item twice",
            IndexError::BadRevocation => "the catalogue has a withdrawn item that is not valid",
        })
    }
}

impl std::error::Error for IndexError {}

impl IndexEntry {
    fn check(&self) -> Result<(), IndexError> {
        let policy = Policy::standard();
        let limits = self.kind.limits();
        let ok = validate_text(&self.name, 64, Lines::One).is_ok()
            && valid_https_url(&self.url, 300)
            && is_sha256_hex(&self.sha256)
            && self.size <= limits.max_total
            && !self.apps.is_empty()
            && self.apps.len() <= MAX_APPS
            && self.tier != Tier::FromFile
            && policy.check_id(self.tier, &self.id).is_ok();
        if ok { Ok(()) } else { Err(IndexError::BadEntry) }
    }
}

#[derive(Deserialize)]
struct Head {
    schema: u32,
}

impl Index {
    /// Reads an index. `now` is unix seconds; `newest_seen` is the highest index version this
    /// machine has accepted before (0 if none).
    pub fn parse(bytes: &[u8], now: u64, newest_seen: u64) -> Result<Self, IndexError> {
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(IndexError::TooLarge);
        }
        let head: Head = serde_json::from_slice(bytes).map_err(|_| IndexError::NotReadable)?;
        if head.schema != SCHEMA {
            return Err(IndexError::UnsupportedSchema(head.schema));
        }
        let index: Index = serde_json::from_slice(bytes).map_err(|_| IndexError::NotReadable)?;
        if index.expires_at <= now {
            return Err(IndexError::Expired);
        }
        if index.version < newest_seen {
            return Err(IndexError::Older);
        }
        if index.entries.len() > MAX_ENTRIES {
            return Err(IndexError::TooManyEntries);
        }
        if index.revoked.len() > MAX_REVOCATIONS {
            return Err(IndexError::TooManyRevocations);
        }
        let mut seen = BTreeSet::new();
        for entry in &index.entries {
            entry.check()?;
            if !seen.insert((&entry.id, entry.version)) {
                return Err(IndexError::DuplicateEntry);
            }
        }
        for r in &index.revoked {
            if validate_text(&r.reason, 200, Lines::One).is_err() {
                return Err(IndexError::BadRevocation);
            }
        }
        Ok(index)
    }

    /// Entries this build may offer: the policy decides which tiers count.
    pub fn admitted<'a>(&'a self, policy: &'a Policy) -> impl Iterator<Item = &'a IndexEntry> + 'a {
        self.entries.iter().filter(move |e| policy.admit(e.tier).is_ok())
    }

    /// True when this id and version has been withdrawn.
    pub fn is_revoked(&self, id: &ExtensionId, version: Version) -> bool {
        self.revoked.iter().any(|r| &r.id == id && r.below.is_none_or(|limit| version < limit))
    }
}

#[cfg(test)]
mod tests;
