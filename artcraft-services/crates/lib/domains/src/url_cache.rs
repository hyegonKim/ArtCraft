//! Process-wide cache backing `Domain::https_url()`.
//!
//! `Url::parse` can't run at compile time, and `Domain` stays free of
//! interior mutability so its constants remain usable in `const` contexts.
//! So each domain's `Url` is parsed on first use and leaked into this map.
//! Domains only come from `domain!` literals, so the set (and the leak) is
//! bounded by the constants in the source code.

use std::collections::BTreeMap;
use std::sync::Mutex;

use url::Url;

static URLS: Mutex<BTreeMap<&'static str, &'static Url>> = Mutex::new(BTreeMap::new());

pub(crate) fn https_url(bare: &'static str, https_with_root_path: &'static str) -> &'static Url {
  // The map is only ever inserted into, so a poisoned lock still holds
  // valid entries.
  let mut urls = URLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  urls.entry(bare).or_insert_with(|| {
    // The domain was validated at compile time, so this parse can't fail.
    let url = Url::parse(https_with_root_path).expect("compile-time validated domain should parse");
    Box::leak(Box::new(url))
  })
}
