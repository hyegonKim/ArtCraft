//! Language packs: `language-data/manifest.toml` (one entry per upstream file of every locale
//! that isn't embedded), the per-locale word checks in `language-data/checks/`, the offline
//! checks CI runs on both, downloading and verifying the files, and building the release assets
//! (`<locale>.tar.zst` + `index.json`). The format is in `docs/language-data-format.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The largest file an entry may name (`craft-proof` reads at most this much per dictionary file).
pub const MAX_FILE: u64 = 64 << 20;

/// The host every entry's `url` is on: `<HOST><owner>/<repo>/<commit>/<path>`, a pinned raw file.
const HOST: &str = "https://raw.githubusercontent.com/";

/// Ok when `url` is `HOST<owner>/<repo>/<commit>/<path…>`: parsed by segment, no empty, `.` or
/// `..` segment, no query or fragment.
///
/// # Errors
///
/// Returns what's wrong with the URL.
pub fn check_pinned_url(url: &str, commit: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix(HOST)
        .ok_or_else(|| format!("url must be a pinned raw file on {HOST}"))?;
    if rest.contains(['?', '#', '\\']) || rest.chars().any(char::is_whitespace) {
        return Err("url has a query, fragment, backslash or space".into());
    }
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 4
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err(
            "url must be <owner>/<repo>/<commit>/<path> with no empty or dot segments".into(),
        );
    }
    if parts.get(2) != Some(&commit) {
        return Err(format!("url is not pinned to commit {commit}"));
    }
    Ok(())
}

/// Whether `name` is a usable file name in `LICENCES/`: `[A-Za-z0-9._-]`, at most 100 characters,
/// not starting or ending with a dot, and not a name Windows reserves for a device. Kept in step
/// with the consumer (craft-libs `font-store`, `licence_name`).
#[must_use]
pub fn safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.starts_with('.')
        && !name.ends_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !windows_reserved(name)
}

/// A name Windows reserves for a device (`CON`, `nul.txt`, `COM1`, `lpt9.md`), with or without an
/// extension, in any case.
fn windows_reserved(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem
                .as_bytes()
                .get(3)
                .is_some_and(|d| (b'1'..=b'9').contains(d)))
}

/// A Licence class (ADR 0004), least restricted first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum Class {
    Embeddable,
    #[serde(rename = "Download-only")]
    DownloadOnly,
    Excluded,
}

impl Class {
    /// The name as the manifest writes it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Embeddable => "Embeddable",
            Class::DownloadOnly => "Download-only",
            Class::Excluded => "Excluded",
        }
    }
}

/// What an entry's file is, and its name in a pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    SpellingAff,
    SpellingDic,
    Hyphenation,
    Licence,
}

/// One `[[file]]` of the manifest.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub locale: String,
    pub kind: Kind,
    pub url: String,
    pub commit: String,
    pub sha256: String,
    pub size: u64,
    pub licence: String,
    pub class: Class,
    #[serde(default)]
    pub name: Option<String>,
    /// The licence statements behind the recorded licence and class, quoted, when the evidence is
    /// weak or contradictory (required when `class` is more restricted than `licence` allows).
    #[serde(default)]
    pub note: Option<String>,
}

impl Entry {
    /// The file's name at the archive root (`LICENCES/<upstream name>` for a licence text).
    #[must_use]
    pub fn pack_path(&self) -> String {
        match self.kind {
            Kind::SpellingAff => "spelling.aff".into(),
            Kind::SpellingDic => "spelling.dic".into(),
            Kind::Hyphenation => "hyphenation.pat".into(),
            Kind::Licence => format!("LICENCES/{}", self.file_name()),
        }
    }

    /// The last segment of the URL.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or_default()
    }
}

/// `language-data/manifest.toml`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub file: Vec<Entry>,
}

impl Manifest {
    /// Parse the manifest text.
    ///
    /// # Errors
    ///
    /// Returns the TOML error when the text is malformed or a field is missing or unknown.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        toml::from_str(text).map_err(|e| format!("manifest.toml: {e}"))
    }

    /// Every locale, sorted.
    #[must_use]
    pub fn locales(&self) -> BTreeSet<&str> {
        self.file.iter().map(|e| e.locale.as_str()).collect()
    }

    /// The entries of `locale`, in manifest order.
    pub fn entries<'a>(&'a self, locale: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        self.file.iter().filter(move |e| e.locale == locale)
    }

    /// The endonym of `locale`: the first entry's `name` that has one, else the tag.
    #[must_use]
    pub fn name<'a>(&'a self, locale: &'a str) -> &'a str {
        self.entries(locale)
            .find_map(|e| e.name.as_deref())
            .unwrap_or(locale)
    }

    /// Where `locale`'s data comes from, for people: the upstream folder of its first data file
    /// (`https://github.com/<owner>/<repo>/tree/<commit>/<dir>`).
    #[must_use]
    pub fn source(&self, locale: &str) -> Option<String> {
        let e = self.entries(locale).find(|e| e.kind != Kind::Licence)?;
        let rest = e.url.strip_prefix(HOST)?;
        let mut parts: Vec<&str> = rest.split('/').collect();
        if parts.len() < 4 {
            return None;
        }
        parts.pop();
        let (repo, path) = parts.split_at(3);
        let [owner, name, commit] = repo else {
            return None;
        };
        let dir = if path.is_empty() {
            String::new()
        } else {
            format!("/{}", path.join("/"))
        };
        Some(format!(
            "https://github.com/{owner}/{name}/tree/{commit}{dir}"
        ))
    }

    /// The Licence class of `locale`'s pack: its most restricted entry's.
    #[must_use]
    pub fn class(&self, locale: &str) -> Class {
        self.entries(locale)
            .map(|e| e.class)
            .max()
            .unwrap_or(Class::Excluded)
    }

    /// The licence of `locale`'s pack: the distinct licences of its data files (licence texts
    /// carry their data file's), joined with `AND`.
    #[must_use]
    pub fn licence(&self, locale: &str) -> String {
        let mut seen: Vec<&str> = Vec::new();
        for e in self.entries(locale) {
            if !seen.contains(&e.licence.as_str()) {
                seen.push(&e.licence);
            }
        }
        let wrap = |l: &str| {
            if seen.len() > 1 && l.contains(" OR ") {
                format!("({l})")
            } else {
                l.to_owned()
            }
        };
        seen.iter()
            .map(|l| wrap(l))
            .collect::<Vec<_>>()
            .join(" AND ")
    }
}

/// `language-data/checks/<locale>.toml`: words a pack must accept and reject, or why the pack is
/// hidden (not published) because `craft-proof` can't read it yet.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    #[serde(default)]
    pub correct: Vec<String>,
    #[serde(default)]
    pub incorrect: Vec<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Every checks file under `dir`, by locale (file stem).
///
/// # Errors
///
/// Returns an error when the folder can't be read or a file isn't valid TOML for [`Checks`].
pub fn load_checks(dir: &Path) -> Result<BTreeMap<String, Checks>, String> {
    let mut out = BTreeMap::new();
    let list = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in list {
        let path = entry.map_err(|e| e.to_string())?.path();
        let (Some(stem), Some("toml")) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.extension().and_then(|s| s.to_str()),
        ) else {
            return Err(format!("{}: not a <locale>.toml file", path.display()));
        };
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let checks: Checks =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        out.insert(stem.to_owned(), checks);
    }
    Ok(out)
}

/// The checks for `locale`: its own file, else its language's (`es-mx` uses `es`).
#[must_use]
pub fn checks_for<'a>(checks: &'a BTreeMap<String, Checks>, locale: &str) -> Option<&'a Checks> {
    checks.get(locale).or_else(|| {
        let lang = locale.split('-').next().unwrap_or(locale);
        checks.get(lang)
    })
}

/// Whether `locale` is published: no Excluded entry, has a spelling or hyphenation file, and its
/// checks don't hide it.
#[must_use]
pub fn published(manifest: &Manifest, checks: &BTreeMap<String, Checks>, locale: &str) -> bool {
    manifest.class(locale) != Class::Excluded
        && manifest.entries(locale).any(|e| e.kind != Kind::Licence)
        && !checks_for(checks, locale).is_some_and(|c| c.hidden)
}

/// The class each allowed licence id implies (ADR 0004). Anything else must be added here, with
/// its class, before an entry may use it.
const LICENCES: &[(&str, Class)] = &[
    ("Apache-2.0", Class::Embeddable),
    ("BSD-2-Clause", Class::Embeddable),
    ("BSD-3-Clause", Class::Embeddable),
    ("CC-BY-4.0", Class::Embeddable),
    ("CC0-1.0", Class::Embeddable),
    ("FSFAP", Class::Embeddable),
    ("ISC", Class::Embeddable),
    ("LPPL-1.3c", Class::Embeddable),
    ("MIT", Class::Embeddable),
    ("MPL-1.1", Class::Embeddable),
    ("MPL-2.0", Class::Embeddable),
    ("Unicode-3.0", Class::Embeddable),
    ("Unlicense", Class::Embeddable),
    ("CC-BY-NC-SA-4.0", Class::Excluded),
    // Licences with no SPDX id; their text is in each pack's LICENCES/.
    ("LicenseRef-BSD-Lebedev", Class::Embeddable),
    ("LicenseRef-hyph-bg", Class::Embeddable),
    ("LicenseRef-SCOWL", Class::Embeddable),
    ("AGPL-3.0-only", Class::DownloadOnly),
    ("AGPL-3.0-or-later", Class::DownloadOnly),
    ("CC-BY-SA-3.0", Class::DownloadOnly),
    ("CC-BY-SA-4.0", Class::DownloadOnly),
    ("GPL-1.0-or-later", Class::DownloadOnly),
    ("GPL-2.0-only", Class::DownloadOnly),
    ("GPL-2.0-or-later", Class::DownloadOnly),
    ("GPL-3.0-only", Class::DownloadOnly),
    ("GPL-3.0-or-later", Class::DownloadOnly),
    ("LGPL-2.0-or-later", Class::DownloadOnly),
    ("LGPL-2.1-only", Class::DownloadOnly),
    ("LGPL-2.1-or-later", Class::DownloadOnly),
    ("LGPL-3.0-only", Class::DownloadOnly),
    ("LGPL-3.0-or-later", Class::DownloadOnly),
];

/// The least restricted class an SPDX expression allows: `OR` takes the less restricted side,
/// `AND` the more restricted one.
///
/// # Errors
///
/// Returns an error when the expression doesn't parse or names a licence not in the allow-list.
pub fn licence_class(expr: &str) -> Result<Class, String> {
    let parsed = spdx::Expression::parse(expr).map_err(|e| format!("{expr:?}: {e}"))?;
    let mut stack: Vec<Class> = Vec::new();
    for node in parsed.iter() {
        match node {
            spdx::expression::ExprNode::Req(r) => {
                if r.req.addition.is_some() {
                    return Err(format!("{expr:?}: WITH exceptions are not allowed"));
                }
                let id = r.req.license.to_string();
                let class = LICENCES
                    .iter()
                    .find(|(l, _)| *l == id)
                    .map(|(_, c)| *c)
                    .ok_or_else(|| format!("{expr:?}: {id} is not in the licence allow-list"))?;
                stack.push(class);
            }
            spdx::expression::ExprNode::Op(op) => {
                let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                    return Err(format!("{expr:?}: malformed"));
                };
                stack.push(match op {
                    spdx::expression::Operator::And => a.max(b),
                    spdx::expression::Operator::Or => a.min(b),
                });
            }
        }
    }
    match stack.as_slice() {
        [c] => Ok(*c),
        _ => Err(format!("{expr:?}: malformed")),
    }
}

/// The licence ids an SPDX expression names (`MIT`, `LicenseRef-…`), each once.
///
/// # Errors
///
/// Returns an error when the expression doesn't parse.
pub fn licence_ids(expr: &str) -> Result<BTreeSet<String>, String> {
    let parsed = spdx::Expression::parse(expr).map_err(|e| format!("{expr:?}: {e}"))?;
    Ok(parsed
        .requirements()
        .map(|r| r.req.license.to_string())
        .collect())
}

/// The licence ids `locale`'s data files are used under, that its archive must carry a full text
/// of: an SPDX id's standard text from `language-data/licences/`, a `LicenseRef-` id's from a
/// `licence` entry that names it.
#[must_use]
pub fn data_licence_ids(manifest: &Manifest, locale: &str) -> BTreeSet<String> {
    manifest
        .entries(locale)
        .filter(|e| e.kind != Kind::Licence)
        .filter_map(|e| licence_ids(&e.licence).ok())
        .flatten()
        .collect()
}

/// Every problem with the manifest, the checks files and the standard licence texts (`texts`, by
/// SPDX id) that needs no network.
#[must_use]
pub fn check(
    manifest: &Manifest,
    checks: &BTreeMap<String, Checks>,
    texts: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for (n, e) in manifest.file.iter().enumerate() {
        let at = format!("file {} ({} {:?})", n + 1, e.locale, e.kind);
        if let Err(err) = crate::locale::check_locale_name(&e.locale) {
            errors.push(format!("{at}: {err}"));
        }
        if e.locale == "en" || e.locale.starts_with("en-") {
            errors.push(format!("{at}: English is embedded, not a pack"));
        }
        let hex40 = e.commit.len() == 40 && e.commit.bytes().all(|b| b.is_ascii_hexdigit());
        if !hex40 {
            errors.push(format!("{at}: commit must be a full 40-digit sha"));
        }
        if let Err(err) = check_pinned_url(&e.url, &e.commit) {
            errors.push(format!("{at}: {err}"));
        }
        let hex64 = e.sha256.len() == 64
            && e.sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex64 {
            errors.push(format!("{at}: sha256 must be 64 lowercase hex digits"));
        }
        if e.size == 0 || e.size > MAX_FILE {
            errors.push(format!("{at}: size {} is outside 1..={MAX_FILE}", e.size));
        }
        if e.licence.trim().is_empty() {
            errors.push(format!("{at}: no licence"));
        } else {
            match licence_class(&e.licence) {
                Ok(implied) if e.class < implied => errors.push(format!(
                    "{at}: class {} but {} is at best {}",
                    e.class.as_str(),
                    e.licence,
                    implied.as_str()
                )),
                Ok(implied)
                    if e.class > implied
                        && e.kind != Kind::Licence
                        && e.note.as_deref().is_none_or(|n| n.trim().is_empty()) =>
                {
                    errors.push(format!(
                        "{at}: class {} is more restricted than {} needs: quote the evidence in `note`",
                        e.class.as_str(),
                        e.licence
                    ));
                }
                Ok(_) => {}
                Err(err) => errors.push(format!("{at}: {err}")),
            }
        }
        let key = match e.kind {
            Kind::Licence => e.pack_path(),
            other => format!("{other:?}"),
        };
        if !seen.insert((e.locale.clone(), key.clone())) {
            errors.push(format!("{at}: duplicate {key} for {}", e.locale));
        }
        if e.kind == Kind::Licence && !safe_file_name(e.file_name()) {
            errors.push(format!(
                "{at}: licence file name must be [A-Za-z0-9._-], not starting with '.'"
            ));
        }
    }
    for locale in manifest.locales() {
        let has = |k: Kind| manifest.entries(locale).any(|e| e.kind == k);
        let spelling = has(Kind::SpellingAff) || has(Kind::SpellingDic);
        if spelling && !(has(Kind::SpellingAff) && has(Kind::SpellingDic)) {
            errors.push(format!(
                "{locale}: spelling needs both spelling-aff and spelling-dic"
            ));
        }
        if !has(Kind::Licence) {
            errors.push(format!(
                "{locale}: needs a licence entry (the upstream notice)"
            ));
        }
        for id in data_licence_ids(manifest, locale) {
            let covered = if id.starts_with("LicenseRef-") {
                manifest.entries(locale).any(|e| {
                    e.kind == Kind::Licence
                        && licence_ids(&e.licence).is_ok_and(|ids| ids.contains(&id))
                })
            } else {
                texts.contains_key(&id)
            };
            if !covered && manifest.class(locale) != Class::Excluded {
                errors.push(format!(
                    "{locale}: no full text of {id} (language-data/licences/{id}.txt, or a licence entry naming a LicenseRef)"
                ));
            }
            let clash = manifest
                .entries(locale)
                .any(|e| e.kind == Kind::Licence && e.file_name() == format!("{id}.txt"));
            if clash {
                errors.push(format!(
                    "{locale}: a licence entry is named {id}.txt, like the standard text"
                ));
            }
        }
        if !spelling && !has(Kind::Hyphenation) {
            errors.push(format!("{locale}: no spelling or hyphenation file"));
        }
        if manifest
            .entries(locale)
            .filter(|e| e.name.is_some())
            .count()
            == 0
        {
            errors.push(format!("{locale}: no entry gives the locale's name"));
        }
        let class = manifest.class(locale);
        match checks_for(checks, locale) {
            None if spelling && class != Class::Excluded => {
                errors.push(format!("{locale}: no language-data/checks/{locale}.toml"));
            }
            Some(c) if c.hidden && c.reason.as_deref().is_none_or(|r| r.trim().is_empty()) => {
                errors.push(format!("{locale}: hidden needs a reason"));
            }
            Some(c) if !c.hidden && spelling && (c.correct.len() < 5 || c.incorrect.len() < 3) => {
                errors.push(format!(
                    "{locale}: checks need at least 5 correct and 3 incorrect words"
                ));
            }
            Some(c)
                if c.correct
                    .iter()
                    .chain(&c.incorrect)
                    .any(|w| w.trim().is_empty() || w.trim() != w) =>
            {
                errors.push(format!(
                    "{locale}: a check word is empty or has spaces around it"
                ));
            }
            _ => {}
        }
    }
    for id in texts.keys() {
        if !LICENCES.iter().any(|(l, _)| l == id) || id.starts_with("LicenseRef-") {
            errors.push(format!(
                "licences/{id}.txt: not an SPDX id in the allow-list"
            ));
        }
    }
    for locale in checks.keys() {
        if !manifest.locales().contains(locale.as_str()) {
            errors.push(format!(
                "checks/{locale}.toml: no such locale in the manifest"
            ));
        }
    }
    errors
}

/// Lowercase hex SHA-256 of `data`.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(data) {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Check `data` against the entry's size and sha256.
///
/// # Errors
///
/// Returns an error naming the entry when the size or the hash differ.
pub fn verify(e: &Entry, data: &[u8]) -> Result<(), String> {
    if data.len() as u64 != e.size {
        return Err(format!(
            "{}: {} bytes, manifest says {}",
            e.url,
            data.len(),
            e.size
        ));
    }
    let got = sha256_hex(data);
    if got != e.sha256 {
        return Err(format!(
            "{}: sha256 {got}, manifest says {}",
            e.url, e.sha256
        ));
    }
    Ok(())
}

/// Where `e` is kept in `cache`: named by its sha256, so a changed pin never reuses a stale file.
#[must_use]
pub fn cache_path(cache: &Path, e: &Entry) -> PathBuf {
    cache.join(&e.sha256)
}

/// The file of `e` from `cache`, verified; downloaded first when it's missing and `download`.
///
/// # Errors
///
/// Returns an error when the file is missing (and `download` is false), the download fails, it
/// exceeds [`MAX_FILE`], or its size or hash differ from the manifest.
pub fn fetch(e: &Entry, cache: &Path, download: bool) -> Result<Vec<u8>, String> {
    let path = cache_path(cache, e);
    if let Ok(data) = read_capped(&path)
        && verify(e, &data).is_ok()
    {
        return Ok(data);
    }
    if !download {
        return Err(format!(
            "{}: not in {} (run `language-packs fetch`)",
            e.url,
            cache.display()
        ));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .into();
    let data = agent
        .get(&e.url)
        .call()
        .map_err(|err| format!("{}: {err}", e.url))?
        .body_mut()
        .with_config()
        .limit(MAX_FILE)
        .read_to_vec()
        .map_err(|err| format!("{}: {err}", e.url))?;
    verify(e, &data)?;
    std::fs::create_dir_all(cache).map_err(|err| format!("{}: {err}", cache.display()))?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, &data).map_err(|err| format!("{}: {err}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(data)
}

fn read_capped(path: &Path) -> Result<Vec<u8>, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.take(MAX_FILE + 1)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    if buf.len() as u64 > MAX_FILE {
        return Err(format!("{}: larger than {MAX_FILE} bytes", path.display()));
    }
    Ok(buf)
}

/// `pack.toml` at an archive's root.
#[derive(Debug, Serialize)]
struct PackToml<'a> {
    locale: &'a str,
    name: &'a str,
    licence: String,
    class: &'static str,
    version: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    upstream: Option<String>,
    /// Standard licence texts in `LICENCES/` (`<SPDX id>.txt`).
    licence_texts: Vec<String>,
    source: Vec<PackSource<'a>>,
}

#[derive(Debug, Serialize)]
struct PackSource<'a> {
    file: String,
    url: &'a str,
    commit: &'a str,
    sha256: &'a str,
    licence: &'a str,
    class: &'static str,
}

/// One pack in `index.json`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexPack {
    pub locale: String,
    pub name: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub licence: String,
    pub class: String,
    pub spelling: bool,
    pub hyphenation: bool,
    /// Where the data comes from, for people (the upstream folder at its pinned commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// `index.json`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Index {
    pub format: u32,
    pub release: String,
    pub packs: Vec<IndexPack>,
}

/// The `<locale>.tar.zst` of `locale`: `files` are its entries' bytes, in manifest order, and
/// `texts` the standard licence texts by SPDX id (the ones its data files use go in `LICENCES/`).
/// Deterministic: sorted names, zero times and owners, mode 0644.
///
/// # Errors
///
/// Returns an error when a licence text is missing, two files share a name, or writing the tar or
/// compressing fails.
pub fn archive(
    manifest: &Manifest,
    locale: &str,
    files: &[(&Entry, Vec<u8>)],
    texts: &BTreeMap<String, Vec<u8>>,
    version: &str,
) -> Result<Vec<u8>, String> {
    let mut standard: Vec<(String, &[u8])> = Vec::new();
    for id in data_licence_ids(manifest, locale) {
        if id.starts_with("LicenseRef-") {
            continue;
        }
        let text = texts
            .get(&id)
            .ok_or_else(|| format!("{locale}: no language-data/licences/{id}.txt"))?;
        standard.push((format!("LICENCES/{id}.txt"), text.as_slice()));
    }
    let pack = PackToml {
        locale,
        name: manifest.name(locale),
        licence: manifest.licence(locale),
        class: manifest.class(locale).as_str(),
        version,
        upstream: manifest.source(locale),
        licence_texts: standard
            .iter()
            .map(|(p, _)| p.trim_start_matches("LICENCES/").to_owned())
            .collect(),
        source: files
            .iter()
            .map(|(e, _)| PackSource {
                file: e.pack_path(),
                url: &e.url,
                commit: &e.commit,
                sha256: &e.sha256,
                licence: &e.licence,
                class: e.class.as_str(),
            })
            .collect(),
    };
    let pack_toml = toml::to_string(&pack).map_err(|e| e.to_string())?;
    let mut items: Vec<(String, &[u8])> = files
        .iter()
        .map(|(e, d)| (e.pack_path(), d.as_slice()))
        .collect();
    items.extend(standard);
    items.push(("pack.toml".into(), pack_toml.as_bytes()));
    items.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some(w) = items
        .windows(2)
        .find(|w| w.first().map(|a| &a.0) == w.get(1).map(|b| &b.0))
    {
        let name = w.first().map_or("", |a| a.0.as_str());
        return Err(format!("{locale}: two files named {name}"));
    }
    let mut tar = tar::Builder::new(Vec::new());
    tar.mode(tar::HeaderMode::Deterministic);
    for (name, data) in items {
        let mut h = tar::Header::new_ustar();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_mtime(0);
        h.set_uid(0);
        h.set_gid(0);
        h.set_entry_type(tar::EntryType::Regular);
        tar.append_data(&mut h, &name, data)
            .map_err(|e| format!("{locale}/{name}: {e}"))?;
    }
    let raw = tar.into_inner().map_err(|e| e.to_string())?;
    zstd::encode_all(raw.as_slice(), 19).map_err(|e| format!("{locale}: zstd: {e}"))
}

/// Build every published pack (or only `only`, when given) into `out`, plus `index.json`, with
/// archive URLs `<base_url><locale>.tar.zst`. Files come from `cache` (downloaded when `download`).
///
/// # Errors
///
/// Returns an error when a file can't be fetched or verified, or writing the output fails.
pub fn build(
    manifest: &Manifest,
    checks: &BTreeMap<String, Checks>,
    texts: &BTreeMap<String, Vec<u8>>,
    opts: &BuildOptions,
) -> Result<Index, String> {
    let locales = manifest.locales();
    if let Some(unknown) = opts.only.iter().find(|o| !locales.contains(o.as_str())) {
        return Err(format!("{unknown}: no such locale in the manifest"));
    }
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    let mut packs = Vec::new();
    for locale in locales {
        if !opts.only.is_empty() && !opts.only.iter().any(|o| o == locale) {
            continue;
        }
        if !published(manifest, checks, locale) {
            if !opts.only.is_empty() {
                return Err(format!("{locale}: hidden or Excluded, not published"));
            }
            continue;
        }
        let mut files = Vec::new();
        for e in manifest.entries(locale) {
            files.push((e, fetch(e, &opts.cache, opts.download)?));
        }
        let data = archive(manifest, locale, &files, texts, &opts.version)?;
        let file = format!("{locale}.tar.zst");
        let path = opts.out.join(&file);
        std::fs::write(&path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
        packs.push(IndexPack {
            locale: locale.to_owned(),
            name: manifest.name(locale).to_owned(),
            version: opts.version.clone(),
            url: format!("{}{file}", opts.base_url),
            sha256: sha256_hex(&data),
            size: data.len() as u64,
            licence: manifest.licence(locale),
            class: manifest.class(locale).as_str().to_owned(),
            spelling: files.iter().any(|(e, _)| e.kind == Kind::SpellingDic),
            hyphenation: files.iter().any(|(e, _)| e.kind == Kind::Hyphenation),
            source: manifest.source(locale),
        });
    }
    let index = Index {
        format: 1,
        release: opts.release.clone(),
        packs,
    };
    let json = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
    let path = opts.out.join("index.json");
    std::fs::write(&path, json + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(index)
}

/// What [`build`] builds and where.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub cache: PathBuf,
    pub out: PathBuf,
    pub release: String,
    pub version: String,
    /// Prefix of every archive URL in `index.json`; ends with `/`.
    pub base_url: String,
    /// Only these locales (all published ones when empty).
    pub only: Vec<String>,
    pub download: bool,
}

/// The default cache folder: `$CRAFT_I18N_CACHE`, else `target/language-data-cache` in the repo.
#[must_use]
pub fn default_cache() -> PathBuf {
    std::env::var_os("CRAFT_I18N_CACHE").map_or_else(
        || crate::repo_root().join("target/language-data-cache"),
        PathBuf::from,
    )
}

/// The standard licence texts in `dir`, by SPDX id (`<id>.txt`).
///
/// # Errors
///
/// Returns an error when the folder can't be read or holds anything but `<id>.txt` files.
pub fn load_licence_texts(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    let list = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in list {
        let path = entry.map_err(|e| e.to_string())?.path();
        let (Some(id), Some("txt")) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.extension().and_then(|s| s.to_str()),
        ) else {
            return Err(format!("{}: not an <SPDX id>.txt file", path.display()));
        };
        out.insert(id.to_owned(), read_capped(&path)?);
    }
    Ok(out)
}

/// The repository's Language pack data: manifest, checks and standard licence texts.
#[derive(Clone, Debug)]
pub struct Repo {
    pub manifest: Manifest,
    pub checks: BTreeMap<String, Checks>,
    pub texts: BTreeMap<String, Vec<u8>>,
}

/// The repository's manifest, checks and licence texts.
///
/// # Errors
///
/// Returns an error when any can't be read or parsed.
pub fn load_repo() -> Result<Repo, String> {
    let root = crate::repo_root().join("language-data");
    let text = std::fs::read_to_string(root.join("manifest.toml"))
        .map_err(|e| format!("manifest.toml: {e}"))?;
    Ok(Repo {
        manifest: Manifest::parse(&text)?,
        checks: load_checks(&root.join("checks"))?,
        texts: load_licence_texts(&root.join("licences"))?,
    })
}
