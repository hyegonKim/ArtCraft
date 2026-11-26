use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn manifest_json(kind: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
    let list: Vec<Value> = files.iter().map(|(p, c)| json!({"path": p, "sha256": hex(c), "size": c.len()})).collect();
    let compat = if kind == "plugin" { json!({"photocraft": {"interface": 1}}) } else { json!({"photocraft": {}}) };
    json!({
        "schema": 1, "id": "org.example.sample", "name": "Sample", "version": "1.0.0", "kind": kind,
        "publisher": {"name": "Example"}, "compat": compat, "files": list
    })
    .to_string()
    .into_bytes()
}

fn zip_of(entries: &[(&str, &[u8])], method: CompressionMethod) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(method);
    for (name, content) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(content).unwrap();
    }
    w.finish().unwrap().into_inner()
}

const THEME: &[u8] = br#"{"schema":1}"#;

fn theme_zip() -> Vec<u8> {
    let m = manifest_json("theme", &[("theme.json", THEME)]);
    zip_of(&[(MANIFEST_NAME, &m), ("theme.json", THEME)], CompressionMethod::Deflated)
}

fn unpack(bytes: &[u8]) -> (tempfile::TempDir, Result<Manifest, UnpackError>) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let r = unpack_into(bytes, &out, Caps::default());
    (dir, r)
}

fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn a_good_theme_unpacks_exactly_its_files() {
    let (dir, r) = unpack(&theme_zip());
    let m = r.unwrap();
    assert_eq!(m.id.as_str(), "org.example.sample");
    let out = dir.path().join("out");
    assert_eq!(files_under(&out), vec!["theme.json"]);
    assert_eq!(fs::read(out.join("theme.json")).unwrap(), THEME);
    assert_eq!(files_under(dir.path()), vec!["out/theme.json"], "nothing was written anywhere else");
}

#[test]
fn stored_and_deflated_both_work() {
    let m = manifest_json("theme", &[("theme.json", THEME)]);
    let stored = zip_of(&[(MANIFEST_NAME, &m), ("theme.json", THEME)], CompressionMethod::Stored);
    assert!(unpack(&stored).1.is_ok());
}

#[test]
fn not_an_archive_and_oversize_input() {
    assert_eq!(unpack(b"").1, Err(UnpackError::NotAnArchive));
    assert_eq!(unpack(b"definitely not a zip file at all").1, Err(UnpackError::NotAnArchive));
    let dir = tempfile::tempdir().unwrap();
    let small = Caps { max_archive: 100, ..Caps::default() };
    assert_eq!(unpack_into(&theme_zip(), &dir.path().join("o"), small).unwrap_err(), UnpackError::TooBig);
    let exact = Caps { max_archive: theme_zip().len() as u64, ..Caps::default() };
    assert!(unpack_into(&theme_zip(), &dir.path().join("o2"), exact).is_ok(), "the limit itself is allowed");
}

#[test]
fn manifest_problems_are_reported_as_such() {
    let z = zip_of(&[("theme.json", THEME)], CompressionMethod::Stored);
    assert_eq!(unpack(&z).1, Err(UnpackError::NoManifest));
    let z = zip_of(&[(MANIFEST_NAME, b"{not json"), ("theme.json", THEME)], CompressionMethod::Stored);
    assert_eq!(unpack(&z).1, Err(UnpackError::Manifest(ManifestError::NotReadable)));
    let big = vec![b' '; MAX_MANIFEST_BYTES + 1];
    let z = zip_of(&[(MANIFEST_NAME, &big)], CompressionMethod::Deflated);
    assert_eq!(unpack(&z).1, Err(UnpackError::Manifest(ManifestError::TooLarge)));
}

#[test]
fn the_archive_must_match_the_manifest_exactly() {
    let m = manifest_json("theme", &[("theme.json", THEME)]);
    let extra = zip_of(&[(MANIFEST_NAME, &m), ("theme.json", THEME), ("payload.exe", b"MZ")], CompressionMethod::Stored);
    let (dir, r) = unpack(&extra);
    assert_eq!(r, Err(UnpackError::Unlisted));
    assert!(!files_under(dir.path()).iter().any(|f| f.contains("payload")), "the stray file was never written");
    let missing = zip_of(&[(MANIFEST_NAME, &m)], CompressionMethod::Stored);
    assert_eq!(unpack(&missing).1, Err(UnpackError::Missing));
}

#[test]
fn hostile_names_are_refused_before_anything_is_written() {
    for (name, expected) in [
        ("../evil.json", PathError::DotSegment),
        ("a/../../evil.json", PathError::DotSegment),
        ("/abs.json", PathError::EmptySegment),
        ("a\\b.json", PathError::BadCharacter),
        ("C:/x.json", PathError::BadCharacter),
        ("nul.json", PathError::ReservedName),
        ("dir./x.json", PathError::TrailingDot),
    ] {
        let m = manifest_json("theme", &[("theme.json", THEME)]);
        let z = zip_of(&[(MANIFEST_NAME, &m), ("theme.json", THEME), (name, b"x")], CompressionMethod::Stored);
        let (dir, r) = unpack(&z);
        assert_eq!(r, Err(UnpackError::BadName(expected)), "{name}");
        assert!(files_under(dir.path()).is_empty(), "{name}: nothing written");
    }
}

#[test]
fn links_are_refused() {
    let m = manifest_json("theme", &[("theme.json", THEME)]);
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let plain = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    w.start_file(MANIFEST_NAME, plain).unwrap();
    w.write_all(&m).unwrap();
    w.add_symlink("theme.json", "/etc/passwd", plain).unwrap();
    let z = w.finish().unwrap().into_inner();
    assert_eq!(unpack(&z).1, Err(UnpackError::Link));
}

#[test]
fn encrypted_entries_are_refused() {
    let mut z = theme_zip();
    // Set bit 0 of the general purpose flags in every local (offset 6) and central (offset 8) header.
    let mut i = 0;
    while i + 10 < z.len() {
        if z[i..i + 4] == [0x50, 0x4b, 0x03, 0x04] {
            z[i + 6] |= 1;
        } else if z[i..i + 4] == [0x50, 0x4b, 0x01, 0x02] {
            z[i + 8] |= 1;
        }
        i += 1;
    }
    assert_eq!(unpack(&z).1, Err(UnpackError::Encrypted));
}

#[test]
fn duplicate_names_never_install() {
    // The archive reader itself collapses repeated names, so the exact error can differ; what matters
    // is that such a package is refused and nothing it holds is accepted.
    let m = manifest_json("resource", &[("a.json", THEME), ("b.json", THEME)]);
    let mut z = zip_of(&[(MANIFEST_NAME, &m), ("a.json", THEME), ("b.json", THEME)], CompressionMethod::Stored);
    let mut i = 0;
    while i + 6 <= z.len() {
        if &z[i..i + 6] == b"b.json" {
            z[i..i + 6].copy_from_slice(b"a.json");
        }
        i += 1;
    }
    assert!(unpack(&z).1.is_err());
}

#[test]
fn too_many_entries_are_refused_before_reading() {
    let m = manifest_json("theme", &[("theme.json", THEME)]);
    let names: Vec<String> = (0..600).map(|i| format!("f{i}.txt")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![(MANIFEST_NAME, &m), ("theme.json", THEME)];
    entries.extend(names.iter().map(|n| (n.as_str(), &b""[..])));
    assert_eq!(unpack(&zip_of(&entries, CompressionMethod::Stored)).1, Err(UnpackError::TooManyEntries));
}

#[test]
fn sizes_and_hashes_must_match_the_manifest() {
    // The manifest claims the file is shorter than it is.
    let lie = json!({
        "schema": 1, "id": "org.example.sample", "name": "Sample", "version": "1.0.0", "kind": "theme",
        "publisher": {"name": "Example"}, "compat": {"photocraft": {}},
        "files": [{"path": "theme.json", "sha256": hex(THEME), "size": THEME.len() - 3}]
    })
    .to_string()
    .into_bytes();
    let z = zip_of(&[(MANIFEST_NAME, &lie), ("theme.json", THEME)], CompressionMethod::Stored);
    let (dir, r) = unpack(&z);
    assert_eq!(r, Err(UnpackError::SizeMismatch));
    assert!(files_under(dir.path()).is_empty(), "a size that disagrees is refused before any file is created");
    // Same size, different bytes.
    let other: Vec<u8> = THEME.iter().map(|b| b ^ 1).collect();
    let m = manifest_json("theme", &[("theme.json", THEME)]);
    let z = zip_of(&[(MANIFEST_NAME, &m), ("theme.json", &other)], CompressionMethod::Stored);
    assert_eq!(unpack(&z).1, Err(UnpackError::HashMismatch));
}

#[test]
fn a_compression_bomb_is_stopped_by_ratio() {
    let zeros = vec![0u8; 8 * 1024 * 1024];
    let m = manifest_json("resource", &[("big.bin", &zeros)]);
    let z = zip_of(&[(MANIFEST_NAME, &m), ("big.bin", &zeros)], CompressionMethod::Deflated);
    assert!(z.len() < 100 * 1024, "the fixture really is a bomb: {} bytes", z.len());
    assert_eq!(unpack(&z).1, Err(UnpackError::Bomb));
    // The same content stored uncompressed is large but honest.
    let stored = zip_of(&[(MANIFEST_NAME, &m), ("big.bin", &zeros)], CompressionMethod::Stored);
    assert!(unpack(&stored).1.is_ok());
}

#[test]
fn a_file_larger_than_it_claims_is_cut_off_not_written_whole() {
    // Central directory and manifest agree on a small size but the data is longer: rewrite the
    // uncompressed size fields of a stored entry to a small number so only streaming can catch it.
    let big = vec![b'x'; 4096];
    let m = manifest_json("resource", &[("data.bin", &big[..16])]);
    let mut z = zip_of(&[(MANIFEST_NAME, &m), ("data.bin", &big)], CompressionMethod::Stored);
    let n = z.len();
    for i in 0..n.saturating_sub(4) {
        let le = |v: u32| v.to_le_bytes();
        if z[i..i + 4] == le(4096) {
            z[i..i + 4].copy_from_slice(&le(16));
        }
    }
    let (dir, r) = unpack(&z);
    assert!(matches!(r, Err(UnpackError::SizeMismatch | UnpackError::Unsupported | UnpackError::NotAnArchive | UnpackError::HashMismatch)), "{r:?}");
    for f in files_under(dir.path()) {
        assert!(fs::metadata(dir.path().join(&f)).unwrap().len() <= 16 + 1, "{f} is bounded");
    }
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("theme.json"), b"precious").unwrap();
    assert_eq!(unpack_into(&theme_zip(), &out, Caps::default()).unwrap_err(), UnpackError::Write);
    assert_eq!(fs::read(out.join("theme.json")).unwrap(), b"precious");
}

#[test]
fn folders_in_the_archive_are_allowed_when_their_names_are_safe() {
    let m = manifest_json("resource", &[("swatches/warm.ase", THEME)]);
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    w.start_file(MANIFEST_NAME, o).unwrap();
    w.write_all(&m).unwrap();
    w.add_directory("swatches/", o).unwrap();
    w.start_file("swatches/warm.ase", o).unwrap();
    w.write_all(THEME).unwrap();
    let z = w.finish().unwrap().into_inner();
    let (dir, r) = unpack(&z);
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(files_under(&dir.path().join("out")), vec!["swatches/warm.ase"]);
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file(MANIFEST_NAME, o).unwrap();
    w.write_all(&m).unwrap();
    w.add_directory("../up/", o).unwrap();
    w.start_file("swatches/warm.ase", o).unwrap();
    w.write_all(THEME).unwrap();
    let z = w.finish().unwrap().into_inner();
    assert!(matches!(unpack(&z).1, Err(UnpackError::BadName(_))), "a folder name is checked like a file name");
}

#[test]
fn every_message_is_plain_language() {
    let all = [
        UnpackError::TooBig,
        UnpackError::NotAnArchive,
        UnpackError::TooManyEntries,
        UnpackError::NoManifest,
        UnpackError::Encrypted,
        UnpackError::Link,
        UnpackError::Unlisted,
        UnpackError::Missing,
        UnpackError::SizeMismatch,
        UnpackError::HashMismatch,
        UnpackError::Bomb,
        UnpackError::Unsupported,
        UnpackError::Write,
    ];
    for e in all {
        let text = e.to_string().to_lowercase();
        for word in ["zip", "archive", "manifest", "sha", "hash", "symlink", "crc", "bomb", "deflate"] {
            assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == word), "{e:?}: {text}");
        }
    }
}
