use super::*;
use serde_json::{Value, json};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NOW: u64 = 1_800_000_000;

fn entry(id: &str, tier: &str) -> Value {
    json!({
        "id": id, "version": "1.0.0", "kind": "theme", "name": "Dark Ocean", "tier": tier,
        "url": "https://github.com/storytold/craft-extensions/releases/download/v1/dark-ocean.zip",
        "sha256": HASH, "size": 2000, "apps": ["photocraft", "launcher"]
    })
}

fn index() -> Value {
    json!({
        "schema": 1, "version": 7, "expires_at": NOW + 3600,
        "entries": [entry("org.storytold.dark-ocean", "first-party"), entry("org.example.warm", "reviewed-third-party")],
        "revoked": [{"id": "org.storytold.bad", "reason": "Broke text"}, {"id": "org.storytold.old", "below": "2.0.0", "reason": "Fixed in 2.0.0"}]
    })
}

fn parse(v: &Value) -> Result<Index, IndexError> {
    Index::parse(v.to_string().as_bytes(), NOW, 0)
}

#[test]
fn a_fresh_index_parses() {
    let i = parse(&index()).unwrap();
    assert_eq!(i.version, 7);
    assert_eq!(i.entries.len(), 2);
    assert_eq!(i.revoked.len(), 2);
}

#[test]
fn size_schema_and_shape() {
    assert_eq!(Index::parse(&vec![b' '; MAX_INDEX_BYTES + 1], NOW, 0), Err(IndexError::TooLarge));
    assert_eq!(Index::parse(b"{", NOW, 0), Err(IndexError::NotReadable));
    let mut v = index();
    v["schema"] = json!(2);
    assert_eq!(parse(&v), Err(IndexError::UnsupportedSchema(2)));
    let mut v = index();
    v["future"] = json!(1);
    assert!(parse(&v).is_ok());
    let mut v = index();
    v.as_object_mut().unwrap().remove("revoked");
    assert!(parse(&v).is_ok(), "no withdrawn items is fine");
}

#[test]
fn an_expired_index_is_refused_at_the_boundary() {
    let mut v = index();
    v["expires_at"] = json!(NOW);
    assert_eq!(parse(&v), Err(IndexError::Expired), "expiring this second counts as expired");
    v["expires_at"] = json!(NOW + 1);
    assert!(parse(&v).is_ok());
    v["expires_at"] = json!(0);
    assert_eq!(parse(&v), Err(IndexError::Expired));
}

#[test]
fn an_older_index_is_refused_but_the_same_or_newer_is_not() {
    let bytes = index().to_string().into_bytes();
    assert_eq!(Index::parse(&bytes, NOW, 8), Err(IndexError::Older));
    assert!(Index::parse(&bytes, NOW, 7).is_ok(), "the same version again is not a rollback");
    assert!(Index::parse(&bytes, NOW, 6).is_ok());
}

#[test]
fn entries_are_checked_hard() {
    let bad_entries: Vec<(&str, Value)> = vec![
        ("name", json!("Dark\u{202e}Ocean")),
        ("url", json!("http://github.com/x.zip")),
        ("url", json!("https://user@github.com/x.zip")),
        ("url", json!("https://github.com/a b.zip")),
        ("url", json!("https://")),
        ("sha256", json!("ABC")),
        ("size", json!(Kind::Theme.limits().max_total + 1)),
        ("apps", json!([])),
    ];
    for (field, value) in bad_entries {
        let mut v = index();
        v["entries"][0][field] = value.clone();
        assert_eq!(parse(&v), Err(IndexError::BadEntry), "{field} {value}");
    }
    let mut v = index();
    v["entries"][0]["apps"] = json!(["Photo Craft"]);
    assert_eq!(parse(&v), Err(IndexError::NotReadable));
}

#[test]
fn the_same_id_and_version_twice_is_refused() {
    let mut v = index();
    v["entries"] = json!([entry("org.storytold.a", "first-party"), entry("org.storytold.a", "first-party")]);
    assert_eq!(parse(&v), Err(IndexError::DuplicateEntry));
    let mut second = entry("org.storytold.a", "first-party");
    second["version"] = json!("1.1.0");
    v["entries"] = json!([entry("org.storytold.a", "first-party"), second]);
    assert!(parse(&v).is_ok(), "two versions of one extension are fine");
}

#[test]
fn only_first_party_entries_may_use_first_party_names() {
    let mut v = index();
    v["entries"] = json!([entry("org.storytold.imposter", "reviewed-third-party")]);
    assert_eq!(parse(&v), Err(IndexError::BadEntry));
    v["entries"] = json!([entry("org.storytold.imposter", "from-file")]);
    assert_eq!(parse(&v), Err(IndexError::BadEntry), "a file is never a catalogue tier");
    // Refused for the tier alone, with an ordinary name no other rule objects to.
    v["entries"] = json!([entry("org.example.ordinary", "from-file")]);
    assert_eq!(parse(&v), Err(IndexError::BadEntry), "a file is never a catalogue tier, whatever its name");
}

#[test]
fn counts_are_bounded() {
    let mut v = index();
    let many: Vec<Value> = (0..=MAX_ENTRIES).map(|i| entry(&format!("org.storytold.e{i}"), "first-party")).collect();
    v["entries"] = Value::Array(many);
    assert_eq!(parse(&v), Err(IndexError::TooManyEntries));
    let mut v = index();
    let many: Vec<Value> = (0..=MAX_REVOCATIONS).map(|i| json!({"id": format!("org.storytold.r{i}"), "reason": "x"})).collect();
    v["revoked"] = Value::Array(many);
    assert_eq!(parse(&v), Err(IndexError::TooManyRevocations));
    let mut v = index();
    let exactly: Vec<Value> = (0..MAX_ENTRIES).map(|i| entry(&format!("org.storytold.e{i}"), "first-party")).collect();
    v["entries"] = Value::Array(exactly);
    assert!(parse(&v).is_ok(), "the limit itself is allowed");
}

#[test]
fn revocations_are_checked() {
    let mut v = index();
    v["revoked"] = json!([{"id": "org.storytold.x", "reason": "a\u{7}b"}]);
    assert_eq!(parse(&v), Err(IndexError::BadRevocation));
    v["revoked"] = json!([{"id": "org.storytold.x", "reason": ""}]);
    assert_eq!(parse(&v), Err(IndexError::BadRevocation));
}

#[test]
fn revocation_covers_all_versions_or_those_below_a_limit() {
    let i = parse(&index()).unwrap();
    let bad = ExtensionId::new("org.storytold.bad").unwrap();
    let old = ExtensionId::new("org.storytold.old").unwrap();
    let other = ExtensionId::new("org.storytold.other").unwrap();
    assert!(i.is_revoked(&bad, Version::new(0, 0, 1)) && i.is_revoked(&bad, Version::new(99, 0, 0)));
    assert!(i.is_revoked(&old, Version::new(1, 9, 9)));
    assert!(!i.is_revoked(&old, Version::new(2, 0, 0)), "the limit itself is not revoked");
    assert!(!i.is_revoked(&old, Version::new(2, 0, 1)));
    assert!(!i.is_revoked(&other, Version::new(1, 0, 0)));
}

#[cfg(not(feature = "third-party"))]
#[test]
fn a_release_build_offers_only_first_party_entries() {
    let i = parse(&index()).unwrap();
    let p = Policy::standard();
    let offered: Vec<&str> = i.admitted(&p).map(|e| e.id.as_str()).collect();
    assert_eq!(offered, vec!["org.storytold.dark-ocean"]);
}

#[test]
fn messages_are_plain_language() {
    for e in [
        IndexError::TooLarge,
        IndexError::NotReadable,
        IndexError::UnsupportedSchema(2),
        IndexError::Expired,
        IndexError::Older,
        IndexError::TooManyEntries,
        IndexError::TooManyRevocations,
        IndexError::BadEntry,
        IndexError::DuplicateEntry,
        IndexError::BadRevocation,
    ] {
        let text = e.to_string().to_lowercase();
        for word in ["index", "schema", "revoked", "sha", "json", "rollback", "manifest"] {
            assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == word), "{e:?}: {text}");
        }
    }
}
