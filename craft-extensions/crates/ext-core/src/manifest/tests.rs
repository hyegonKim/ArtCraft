use super::*;
use serde_json::{Value, json};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn theme() -> Value {
    json!({
        "schema": 1, "id": "org.example.dark-ocean", "name": "Dark Ocean", "version": "1.0.0", "kind": "theme",
        "publisher": {"name": "Example"}, "description": "A calm dark theme.",
        "compat": {"photocraft": {"min_app": "0.9.0"}},
        "files": [{"path": "theme.json", "sha256": HASH, "size": 1200}],
        "permissions": []
    })
}

fn plugin() -> Value {
    let mut v = theme();
    v["id"] = json!("org.example.tone");
    v["kind"] = json!("plugin");
    v["compat"] = json!({"photocraft": {"interface": 1}});
    v["files"] = json!([{"path": "plugin.wasm", "sha256": HASH, "size": 5000}]);
    v
}

fn resource() -> Value {
    let mut v = theme();
    v["id"] = json!("org.example.swatches");
    v["kind"] = json!("resource");
    v["files"] = json!([{"path": "swatches/warm.ase", "sha256": HASH, "size": 900}, {"path": "swatches/cool.ase", "sha256": HASH, "size": 900}]);
    v
}

fn parse(v: &Value) -> Result<Manifest, ManifestError> {
    Manifest::parse(v.to_string().as_bytes())
}

#[test]
fn valid_packages_of_each_kind_parse() {
    let m = parse(&theme()).unwrap();
    assert_eq!(m.id.as_str(), "org.example.dark-ocean");
    assert_eq!(m.kind, Kind::Theme);
    assert_eq!(m.version, Version::new(1, 0, 0));
    assert!(m.file("theme.json").is_some());
    assert_eq!(parse(&plugin()).unwrap().kind, Kind::Plugin);
    assert_eq!(parse(&resource()).unwrap().kind, Kind::Resource);
}

#[test]
fn size_is_capped_before_parsing() {
    let big = vec![b' '; MAX_MANIFEST_BYTES + 1];
    assert_eq!(Manifest::parse(&big), Err(ManifestError::TooLarge));
    assert_eq!(Manifest::parse(b""), Err(ManifestError::NotReadable));
    assert_eq!(Manifest::parse(b"{not json"), Err(ManifestError::NotReadable));
    assert_eq!(Manifest::parse(&[0xff, 0xfe, 0x00]), Err(ManifestError::NotReadable));
    assert_eq!(Manifest::parse(b"[]"), Err(ManifestError::NotReadable));
}

#[test]
fn newer_or_missing_schema_is_refused_before_the_rest_is_read() {
    let mut v = theme();
    v["schema"] = json!(2);
    v["something_new"] = json!({"a": 1});
    v["kind"] = json!("a-kind-this-host-does-not-know");
    assert_eq!(parse(&v), Err(ManifestError::UnsupportedSchema(2)));
    v["schema"] = json!(0);
    assert_eq!(parse(&v), Err(ManifestError::UnsupportedSchema(0)));
    let mut v = theme();
    v.as_object_mut().unwrap().remove("schema");
    assert_eq!(parse(&v), Err(ManifestError::NotReadable));
}

#[test]
fn unknown_fields_are_ignored_and_a_package_cannot_claim_a_tier() {
    let mut v = theme();
    v["tier"] = json!("first-party");
    v["trusted"] = json!(true);
    v["publisher"]["verified"] = json!(true);
    let m = parse(&v).unwrap();
    let again = serde_json::to_string(&m).unwrap();
    assert!(!again.contains("first-party") && !again.contains("trusted") && !again.contains("verified"));
}

#[test]
fn bad_identity_fields_are_refused_with_their_reason() {
    let mut v = theme();
    v["id"] = json!("Not.Valid");
    assert_eq!(parse(&v), Err(ManifestError::NotReadable), "ids are validated while reading");
    let mut v = theme();
    v["name"] = json!("Dark\u{202e}Ocean");
    assert_eq!(parse(&v), Err(ManifestError::Name(TextError::InvisibleFormatting)));
    v["name"] = json!("x".repeat(65));
    assert_eq!(parse(&v), Err(ManifestError::Name(TextError::TooLong)));
    let mut v = theme();
    v["description"] = json!("a\u{7}b");
    assert_eq!(parse(&v), Err(ManifestError::Description(TextError::ControlCharacter)));
    v["description"] = json!("x".repeat(501));
    assert_eq!(parse(&v), Err(ManifestError::Description(TextError::TooLong)));
    v["description"] = json!("");
    assert!(parse(&v).is_ok(), "an empty description is allowed");
    let mut v = theme();
    v["publisher"]["name"] = json!("");
    assert_eq!(parse(&v), Err(ManifestError::Publisher(TextError::Empty)));
    let mut v = theme();
    v["version"] = json!("1.0");
    assert_eq!(parse(&v), Err(ManifestError::NotReadable));
    let mut v = theme();
    v["kind"] = json!("macro");
    assert_eq!(parse(&v), Err(ManifestError::NotReadable));
}

#[test]
fn permissions_must_be_empty_for_every_kind() {
    for mut v in [theme(), plugin(), resource()] {
        v["permissions"] = json!(["files"]);
        assert_eq!(parse(&v), Err(ManifestError::PermissionsNotEmpty));
    }
    let mut v = theme();
    v.as_object_mut().unwrap().remove("permissions");
    assert!(parse(&v).is_ok(), "no permissions field means none");
}

#[test]
fn compat_rules_depend_on_the_kind() {
    let mut v = theme();
    v["compat"] = json!({});
    assert_eq!(parse(&v), Err(ManifestError::NoApps));
    let mut many = serde_json::Map::new();
    for i in 0..=MAX_APPS {
        many.insert(format!("app{}", (b'a' + (i % 26) as u8) as char).repeat(1) + &"x".repeat(i / 26), json!({}));
    }
    let mut v = theme();
    v["compat"] = Value::Object(many);
    assert_eq!(parse(&v), Err(ManifestError::TooManyApps));
    let mut v = plugin();
    v["compat"] = json!({"photocraft": {}});
    assert_eq!(parse(&v), Err(ManifestError::BadCompat), "a plug-in must name its interface version");
    v["compat"] = json!({"photocraft": {"interface": 0}});
    assert_eq!(parse(&v), Err(ManifestError::BadCompat));
    let mut v = theme();
    v["compat"] = json!({"photocraft": {"interface": 1}});
    assert_eq!(parse(&v), Err(ManifestError::BadCompat), "only plug-ins have an interface version");
    let mut v = theme();
    v["compat"] = json!({"Photo Craft": {}});
    assert_eq!(parse(&v), Err(ManifestError::NotReadable));
}

#[test]
fn files_must_match_what_the_kind_needs() {
    let mut v = theme();
    v["files"] = json!([{"path": "other.json", "sha256": HASH, "size": 10}]);
    assert_eq!(parse(&v), Err(ManifestError::MissingRequiredFile));
    v["files"] = json!([{"path": "theme.json", "sha256": HASH, "size": 10}, {"path": "extra.png", "sha256": HASH, "size": 10}]);
    assert_eq!(parse(&v), Err(ManifestError::TooManyFiles));
    let mut v = plugin();
    v["files"] = json!([{"path": "theme.json", "sha256": HASH, "size": 10}]);
    assert_eq!(parse(&v), Err(ManifestError::MissingRequiredFile));
    let mut v = resource();
    v["files"] = json!([]);
    assert_eq!(parse(&v), Err(ManifestError::MissingRequiredFile));
}

#[test]
fn file_paths_hashes_and_sizes_are_checked() {
    let mut v = resource();
    v["files"] = json!([{"path": "../evil.ase", "sha256": HASH, "size": 1}]);
    assert_eq!(parse(&v), Err(ManifestError::Files(PathError::DotSegment)));
    v["files"] = json!([{"path": "a.ase", "sha256": HASH, "size": 1}, {"path": "A.ase", "sha256": HASH, "size": 1}]);
    assert_eq!(parse(&v), Err(ManifestError::Files(PathError::CaseCollision)));
    for bad in [HASH.to_uppercase(), HASH[..63].to_string(), format!("{HASH}0"), "g".repeat(64), String::new()] {
        v["files"] = json!([{"path": "a.ase", "sha256": bad, "size": 1}]);
        assert_eq!(parse(&v), Err(ManifestError::BadHash), "{bad}");
    }
    v["files"] = json!([{"path": "a.ase", "sha256": HASH, "size": Kind::Resource.limits().max_file + 1}]);
    assert_eq!(parse(&v), Err(ManifestError::FileTooBig));
    let big = Kind::Resource.limits().max_file;
    v["files"] = json!([
        {"path": "a.ase", "sha256": HASH, "size": big}, {"path": "b.ase", "sha256": HASH, "size": big},
        {"path": "c.ase", "sha256": HASH, "size": big}, {"path": "d.ase", "sha256": HASH, "size": big},
        {"path": "e.ase", "sha256": HASH, "size": 1}
    ]);
    assert_eq!(parse(&v), Err(ManifestError::TotalTooBig));
    let mut v = theme();
    v["files"][0]["size"] = json!(Kind::Theme.limits().max_file + 1);
    assert_eq!(parse(&v), Err(ManifestError::FileTooBig));
    v["files"][0]["size"] = json!(u64::MAX);
    assert_eq!(parse(&v), Err(ManifestError::FileTooBig));
    let mut v = resource();
    let many: Vec<Value> = (0..=Kind::Resource.limits().max_files).map(|i| json!({"path": format!("f{i}.ase"), "sha256": HASH, "size": 1})).collect();
    v["files"] = Value::Array(many);
    assert_eq!(parse(&v), Err(ManifestError::TooManyFiles));
}

#[test]
fn provenance_is_checked_when_present() {
    let mut v = theme();
    v["provenance"] = json!({"source": "https://github.com/example/dark-ocean", "commit": "0123456789abcdef0123456789abcdef01234567"});
    assert!(parse(&v).is_ok());
    for (source, commit) in [
        ("http://github.com/example/x", "0123456789abcdef0123456789abcdef01234567"),
        ("https://user:pw@github.com/x", "0123456789abcdef0123456789abcdef01234567"),
        ("https://github.com/exa mple", "0123456789abcdef0123456789abcdef01234567"),
        ("https://", "0123456789abcdef0123456789abcdef01234567"),
        ("https://github.com/example/x", "0123456789ABCDEF0123456789abcdef01234567"),
        ("https://github.com/example/x", "abc"),
    ] {
        v["provenance"] = json!({"source": source, "commit": commit});
        assert_eq!(parse(&v), Err(ManifestError::BadProvenance), "{source} {commit}");
    }
}

#[test]
fn every_message_is_plain_language() {
    let all = [
        ManifestError::TooLarge,
        ManifestError::NotReadable,
        ManifestError::UnsupportedSchema(2),
        ManifestError::Id(IdError::BadShape),
        ManifestError::Name(TextError::Empty),
        ManifestError::Description(TextError::TooLong),
        ManifestError::Publisher(TextError::NothingVisible),
        ManifestError::NoApps,
        ManifestError::TooManyApps,
        ManifestError::BadCompat,
        ManifestError::TooManyFiles,
        ManifestError::FileTooBig,
        ManifestError::TotalTooBig,
        ManifestError::BadHash,
        ManifestError::Files(PathError::DotSegment),
        ManifestError::MissingRequiredFile,
        ManifestError::PermissionsNotEmpty,
        ManifestError::BadProvenance,
    ];
    let jargon = ["abi", "wasm", "sandbox", "adapter", "serde", "json", "utf", "schema", "sha", "hash", "manifest", "regex", "panic"];
    for e in all {
        let text = e.to_string().to_lowercase();
        for word in jargon {
            assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == word), "{e:?} says {word}: {text}");
        }
    }
}
