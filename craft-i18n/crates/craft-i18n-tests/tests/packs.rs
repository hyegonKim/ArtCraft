//! Language packs (`language-data/manifest.toml`, `language-data/checks/`): the offline checks
//! every CI run makes, and two network checks that run only with `CRAFT_I18N_FETCH=1` (the
//! scheduled and release workflows): every file downloads with its sha256 and size, and every
//! published pack loads in `craft-proof` and accepts and rejects its check words.

use std::collections::BTreeMap;

use craft_i18n_tests::packs::{self, Checks, Class, Entry, Kind, Manifest};

#[test]
fn manifest_and_checks_are_valid() {
    let repo = packs::load_repo().unwrap();
    assert!(
        repo.manifest.file.len() > 10,
        "manifest.toml is nearly empty"
    );
    let errors = packs::check(&repo.manifest, &repo.checks, &repo.texts);
    assert!(errors.is_empty(), "language packs:\n{}", errors.join("\n"));
}

#[test]
fn excluded_locales_are_never_published() {
    let packs::Repo {
        manifest, checks, ..
    } = packs::load_repo().unwrap();
    for e in manifest.file.iter().filter(|e| e.class == Class::Excluded) {
        assert!(
            !packs::published(&manifest, &checks, &e.locale),
            "{} has an Excluded file but is published",
            e.locale
        );
    }
}

fn fetching() -> bool {
    if std::env::var("CRAFT_I18N_FETCH").as_deref() == Ok("1") {
        true
    } else {
        eprintln!("skipped: set CRAFT_I18N_FETCH=1 to download the Language pack files");
        false
    }
}

/// Network: every file downloads (or is cached) with the manifest's size and sha256.
#[test]
fn every_file_matches_its_pin() {
    if !fetching() {
        return;
    }
    let manifest = packs::load_repo().unwrap().manifest;
    let cache = packs::default_cache();
    let errors: Vec<String> = manifest
        .file
        .iter()
        .filter_map(|e| packs::fetch(e, &cache, true).err())
        .collect();
    assert!(errors.is_empty(), "downloads:\n{}", errors.join("\n"));
}

fn file(manifest: &Manifest, locale: &str, kind: Kind) -> Option<Vec<u8>> {
    let e = manifest.entries(locale).find(|e| e.kind == kind)?;
    Some(packs::fetch(e, &packs::default_cache(), true).unwrap())
}

/// What's wrong with `locale`'s spelling pair against its checks, or the load error.
fn spelling_problems(aff: &[u8], dic: &[u8], checks: &Checks) -> Result<Vec<String>, String> {
    let (aff, dic) = craft_proof::hunspell::decode(aff, dic)?;
    let d = craft_proof::affix::AffixDict::from_hunspell(&aff, &dic, &[])?;
    let mut problems = Vec::new();
    for w in &checks.correct {
        if !d.check(w) {
            problems.push(format!("rejects {w:?}"));
        }
    }
    for w in &checks.incorrect {
        if d.check(w) {
            problems.push(format!("accepts {w:?}"));
        }
    }
    Ok(problems)
}

/// Network: every published pack's dictionary loads through craft-proof's affix engine and gets
/// its check words right, and its hyphenation patterns compile. Hidden packs are tried too and
/// reported (not failed), so the report says when one could be published.
#[test]
fn packs_load_and_check() {
    if !fetching() {
        return;
    }
    let packs::Repo {
        manifest, checks, ..
    } = packs::load_repo().unwrap();
    let mut errors = Vec::new();
    for locale in manifest.locales() {
        if manifest.class(locale) == Class::Excluded {
            continue;
        }
        let published = packs::published(&manifest, &checks, locale);
        if let (Some(aff), Some(dic), Some(c)) = (
            file(&manifest, locale, Kind::SpellingAff),
            file(&manifest, locale, Kind::SpellingDic),
            packs::checks_for(&checks, locale),
        ) {
            match (spelling_problems(&aff, &dic, c), published) {
                (Ok(p), true) if p.is_empty() => eprintln!("{locale}: ok"),
                (Ok(p), true) => errors.push(format!("{locale}: {}", p.join(", "))),
                (Err(e), true) => errors.push(format!("{locale}: doesn't load: {e}")),
                (Ok(p), false) if p.is_empty() => {
                    eprintln!("{locale}: hidden, but loads and passes now: consider publishing it");
                }
                (Ok(p), false) => eprintln!("{locale}: hidden; loads, {}", p.join(", ")),
                (Err(e), false) => eprintln!("{locale}: hidden; {e}"),
            }
        }
        if published && let Some(pat) = file(&manifest, locale, Kind::Hyphenation) {
            match String::from_utf8(pat) {
                Ok(text) if !craft_proof::patterns::Patterns::parse(&text).is_empty() => {}
                Ok(_) => errors.push(format!("{locale}: hyphenation has no patterns")),
                Err(e) => errors.push(format!("{locale}: hyphenation is not UTF-8: {e}")),
            }
        }
    }
    assert!(errors.is_empty(), "packs:\n{}", errors.join("\n"));
}

mod fixtures {
    use super::*;

    const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const COMMIT: &str = "4f1df64b2e8e14751aa7c2205d170ca016071315";

    fn entry(locale: &str, kind: Kind, path: &str, licence: &str, class: Class) -> Entry {
        Entry {
            locale: locale.into(),
            kind,
            url: format!(
                "https://raw.githubusercontent.com/LibreOffice/dictionaries/{COMMIT}/{path}"
            ),
            commit: COMMIT.into(),
            sha256: SHA.into(),
            size: 10,
            licence: licence.into(),
            class,
            name: Some("Nederlands".into()),
            note: None,
        }
    }

    fn nl() -> Vec<Entry> {
        vec![
            entry(
                "nl",
                Kind::SpellingAff,
                "nl_NL/nl_NL.aff",
                "BSD-3-Clause",
                Class::Embeddable,
            ),
            entry(
                "nl",
                Kind::SpellingDic,
                "nl_NL/nl_NL.dic",
                "BSD-3-Clause",
                Class::Embeddable,
            ),
            entry(
                "nl",
                Kind::Licence,
                "nl_NL/LICENSE.txt",
                "BSD-3-Clause",
                Class::Embeddable,
            ),
        ]
    }

    fn words() -> BTreeMap<String, Checks> {
        let c = Checks {
            correct: ["a", "b", "c", "d", "e"].map(String::from).to_vec(),
            incorrect: ["x", "y", "z"].map(String::from).to_vec(),
            ..Checks::default()
        };
        BTreeMap::from([("nl".to_owned(), c)])
    }

    /// Standard licence texts the fixtures have: BSD-3-Clause and MIT only.
    fn texts() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("BSD-3-Clause".to_owned(), b"BSD text\n".to_vec()),
            ("MIT".to_owned(), b"MIT text\n".to_vec()),
        ])
    }

    fn errors(file: Vec<Entry>, checks: &BTreeMap<String, Checks>) -> Vec<String> {
        packs::check(&Manifest { file }, checks, &texts())
    }

    #[test]
    fn clean() {
        assert_eq!(errors(nl(), &words()), Vec::<String>::new());
    }

    #[test]
    fn each_failure() {
        type Edit = fn(&mut Vec<Entry>, &mut BTreeMap<String, Checks>);
        let cases: [(Edit, &str); 27] = [
            (|f, _| f[0].licence.clear(), "no licence"),
            (
                |f, _| f[0].licence = "BSD-3-Clause OR".into(),
                "BSD-3-Clause OR",
            ),
            (
                |f, _| f[0].licence = "WTFPL".into(),
                "not in the licence allow-list",
            ),
            (
                |f, _| f[0].licence = "GPL-2.0-only".into(),
                "is at best Download-only",
            ),
            (|f, _| f[0].locale = "nl-NL".into(), "write it as"),
            (
                |f, _| f[0].url = "http://example.com/nl.aff".into(),
                "pinned raw file",
            ),
            (|f, _| f[0].commit = "abc".into(), "40-digit"),
            (
                |f, _| f[0].url = f[0].url.replace(COMMIT, "main"),
                "not pinned to commit",
            ),
            (|f, _| f[0].sha256 = "ABC".into(), "64 lowercase hex"),
            (|f, _| f[0].size = packs::MAX_FILE + 1, "outside 1..="),
            (|f, _| f[0].size = 0, "outside 1..="),
            (|f, _| f.push(f[0].clone()), "duplicate SpellingAff"),
            (
                |f, _| f.push(f[2].clone()),
                "duplicate LICENCES/LICENSE.txt",
            ),
            (
                |f, _| {
                    f.remove(1);
                },
                "needs both",
            ),
            (
                |f, _| {
                    f.remove(2);
                },
                "needs a licence entry",
            ),
            // The commit only in a query string, not as the third path segment.
            (
                |f, _| {
                    f[0].url = format!(
                        "https://raw.githubusercontent.com/LibreOffice/dictionaries/main/x.aff?/{COMMIT}/"
                    );
                },
                "query",
            ),
            (
                |f, _| {
                    f[0].url = format!("https://raw.githubusercontent.com/a/b/main/{COMMIT}/x.aff");
                },
                "not pinned to commit",
            ),
            (
                |f, _| f[0].url = f[0].url.replace("nl_NL/", "../"),
                "dot segments",
            ),
            (
                |f, _| {
                    f[0].url =
                        format!("https://raw.githubusercontent.com.evil.example/a/b/{COMMIT}/x")
                },
                "pinned raw file",
            ),
            (
                |f, _| f[2].url = f[2].url.replace("LICENSE.txt", "LICENSE%20x.txt"),
                "[A-Za-z0-9._-]",
            ),
            (
                |f, _| f[2].url = f[2].url.replace("LICENSE.txt", ".hidden"),
                "[A-Za-z0-9._-]",
            ),
            (
                |_, c| {
                    if let Some(nl) = c.get_mut("nl") {
                        nl.correct[0] = " ".into();
                    }
                },
                "check word is empty",
            ),
            // GPL data with only an MIT text: GPL is not covered.
            (
                |f, _| {
                    for e in f.iter_mut() {
                        e.class = Class::DownloadOnly;
                        e.licence = "GPL-2.0-only".into();
                    }
                    f[2].licence = "MIT".into();
                },
                "no full text of GPL-2.0-only",
            ),
            // Weak evidence: a stricter class than the licence needs must say why.
            (
                |f, _| {
                    f[0].class = Class::DownloadOnly;
                },
                "quote the evidence in `note`",
            ),
            // A LicenseRef needs a licence entry naming it.
            (
                |f, _| {
                    f[0].licence = "LicenseRef-BSD-Lebedev".into();
                },
                "no full text of LicenseRef-BSD-Lebedev",
            ),
            (|_, c| c.clear(), "no language-data/checks/nl.toml"),
            (
                |_, c| {
                    c.insert("fr".into(), Checks::default());
                },
                "checks/fr.toml: no such locale",
            ),
        ];
        for (edit, want) in cases {
            let (mut f, mut c) = (nl(), words());
            edit(&mut f, &mut c);
            let got = errors(f, &c);
            assert!(got.iter().any(|e| e.contains(want)), "{want}: got {got:?}");
        }
        let mut c = words();
        if let Some(nl) = c.get_mut("nl") {
            nl.hidden = true;
        }
        assert!(
            errors(nl(), &c)
                .iter()
                .any(|e| e.contains("hidden needs a reason"))
        );
        if let Some(nl) = c.get_mut("nl") {
            nl.hidden = false;
            nl.correct.clear();
        }
        assert!(errors(nl(), &c).iter().any(|e| e.contains("at least 5")));
        let mut en = nl();
        for e in &mut en {
            e.locale = "en-gb".into();
        }
        // A hyphenation-only GPL pack with no licence entry fails.
        let mut hyph = entry(
            "nl",
            Kind::Hyphenation,
            "x/hyph-nl.tex",
            "GPL-2.0-only",
            Class::DownloadOnly,
        );
        hyph.url = format!(
            "https://raw.githubusercontent.com/hyphenation/tex-hyphen/{COMMIT}/hyph-nl.tex"
        );
        let got = errors(vec![hyph], &BTreeMap::new());
        assert!(
            got.iter().any(|e| e.contains("needs a licence entry")),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|e| e.contains("no full text of GPL-2.0-only")),
            "{got:?}"
        );
        assert!(
            errors(en, &BTreeMap::new())
                .iter()
                .any(|e| e.contains("English is embedded"))
        );
    }

    #[test]
    fn licence_classes() {
        for (expr, want) in [
            ("MIT", Class::Embeddable),
            (
                "GPL-2.0-only OR LGPL-2.1-only OR MPL-1.1",
                Class::Embeddable,
            ),
            ("GPL-2.0-only AND CC-BY-4.0", Class::DownloadOnly),
            (
                "(MIT OR GPL-3.0-only) AND LGPL-3.0-only",
                Class::DownloadOnly,
            ),
            ("LicenseRef-BSD-Lebedev", Class::Embeddable),
            ("CC-BY-SA-3.0", Class::DownloadOnly),
        ] {
            assert_eq!(packs::licence_class(expr), Ok(want), "{expr}");
        }
        for bad in [
            "",
            "MIT AND",
            "MIT+",
            "GPL-2.0-only WITH Classpath-exception-2.0",
            "Nope",
        ] {
            assert!(packs::licence_class(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_excluded_or_hidden_locale_is_not_published() {
        let m = Manifest { file: nl() };
        let mut c = words();
        assert!(packs::published(&m, &c, "nl"));
        if let Some(nl) = c.get_mut("nl") {
            nl.hidden = true;
        }
        assert!(!packs::published(&m, &c, "nl"));
        let mut f = nl();
        f[1].class = Class::Excluded;
        let m = Manifest { file: f };
        assert!(!packs::published(&m, &words(), "nl"));
        assert_eq!(m.class("nl"), Class::Excluded);
    }

    #[test]
    fn regional_locales_use_their_language_checks() {
        let c = words();
        assert!(packs::checks_for(&c, "nl-be").is_some());
        assert!(packs::checks_for(&c, "fr").is_none());
    }

    #[test]
    fn pack_licence_joins_distinct_licences() {
        let mut f = nl();
        f.push(entry(
            "nl",
            Kind::Hyphenation,
            "x/hyph-nl.tex",
            "MIT OR LPPL-1.3c",
            Class::Embeddable,
        ));
        let m = Manifest { file: f };
        assert_eq!(m.licence("nl"), "BSD-3-Clause AND (MIT OR LPPL-1.3c)");
        assert_eq!(m.name("nl"), "Nederlands");
    }

    #[test]
    fn verify_checks_size_and_hash() {
        let mut e = entry(
            "nl",
            Kind::Licence,
            "nl_NL/LICENSE.txt",
            "MIT",
            Class::Embeddable,
        );
        e.size = 5;
        e.sha256 = packs::sha256_hex(b"hello");
        assert_eq!(packs::verify(&e, b"hello"), Ok(()));
        assert!(packs::verify(&e, b"hellO").unwrap_err().contains("sha256"));
        assert!(packs::verify(&e, b"hello!").unwrap_err().contains("bytes"));
    }

    /// The archive holds the contract layout, is the same bytes every time, and `build` writes it
    /// with an index whose URL, sha256 and size match.
    #[test]
    fn archive_and_index() {
        let dir = std::env::temp_dir().join(format!("craft-i18n-packs-{}", std::process::id()));
        let cache = dir.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let mut file = Vec::new();
        for (kind, path, data) in [
            (Kind::SpellingAff, "nl_NL/nl_NL.aff", &b"SET UTF-8\n"[..]),
            (Kind::SpellingDic, "nl_NL/nl_NL.dic", &b"1\nhuis\n"[..]),
            (
                Kind::Hyphenation,
                "x/hyph-nl.tex",
                &b"\\patterns{1ba}\n"[..],
            ),
            (Kind::Licence, "nl_NL/LICENSE.txt", &b"BSD\n"[..]),
        ] {
            let mut e = entry("nl", kind, path, "BSD-3-Clause", Class::Embeddable);
            e.sha256 = packs::sha256_hex(data);
            e.size = data.len() as u64;
            std::fs::write(packs::cache_path(&cache, &e), data).unwrap();
            file.push(e);
        }
        let m = Manifest { file };
        let opts = packs::BuildOptions {
            cache: cache.clone(),
            out: dir.join("out"),
            release: "language-packs-test".into(),
            version: "abc123".into(),
            base_url: "http://127.0.0.1:8765/".into(),
            only: vec![],
            download: false,
        };
        let index = packs::build(&m, &words(), &texts(), &opts).unwrap();
        let [p] = index.packs.as_slice() else {
            panic!("{index:?}")
        };
        assert_eq!(p.url, "http://127.0.0.1:8765/nl.tar.zst");
        assert_eq!(
            p.source.as_deref(),
            Some(
                format!("https://github.com/LibreOffice/dictionaries/tree/{COMMIT}/nl_NL").as_str()
            )
        );
        assert!(p.spelling && p.hyphenation);
        assert_eq!(
            (p.class.as_str(), p.licence.as_str()),
            ("Embeddable", "BSD-3-Clause")
        );
        let bytes = std::fs::read(dir.join("out/nl.tar.zst")).unwrap();
        assert_eq!(
            (packs::sha256_hex(&bytes), bytes.len() as u64),
            (p.sha256.clone(), p.size)
        );
        let json: packs::Index =
            serde_json::from_str(&std::fs::read_to_string(dir.join("out/index.json")).unwrap())
                .unwrap();
        assert_eq!(json, index);
        // Deterministic.
        packs::build(&m, &words(), &texts(), &opts).unwrap();
        assert_eq!(std::fs::read(dir.join("out/nl.tar.zst")).unwrap(), bytes);
        // Layout.
        let raw = zstd::decode_all(bytes.as_slice()).unwrap();
        let mut ar = tar::Archive::new(raw.as_slice());
        let mut names = BTreeMap::new();
        for f in ar.entries().unwrap() {
            let mut f = f.unwrap();
            let mut s = String::new();
            std::io::Read::read_to_string(&mut f, &mut s).unwrap();
            names.insert(f.path().unwrap().to_string_lossy().into_owned(), s);
        }
        let keys: Vec<&str> = names.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "LICENCES/BSD-3-Clause.txt",
                "LICENCES/LICENSE.txt",
                "hyphenation.pat",
                "pack.toml",
                "spelling.aff",
                "spelling.dic"
            ]
        );
        let pack = &names["pack.toml"];
        assert!(
            pack.contains("locale = \"nl\"") && pack.contains("class = \"Embeddable\""),
            "{pack}"
        );
        assert!(pack.contains("nl_NL/nl_NL.dic"), "{pack}");
        assert!(
            pack.contains("upstream = \"https://github.com/LibreOffice/"),
            "{pack}"
        );
        assert_eq!(names["LICENCES/BSD-3-Clause.txt"], "BSD text\n");
        // A data licence with no standard text can't be archived.
        let bare = BTreeMap::new();
        assert!(
            packs::build(&m, &words(), &bare, &opts)
                .unwrap_err()
                .contains("BSD-3-Clause.txt")
        );
        // `only` with an unknown locale is an error.
        let unknown = packs::BuildOptions {
            only: vec!["xx".into()],
            ..opts.clone()
        };
        assert!(
            packs::build(&m, &words(), &texts(), &unknown)
                .unwrap_err()
                .contains("no such locale")
        );
        // A missing cache file fails without downloading.
        std::fs::remove_file(cache.join(&m.file[0].sha256)).unwrap();
        assert!(
            packs::build(&m, &words(), &texts(), &opts)
                .unwrap_err()
                .contains("not in")
        );
        // `only` refuses a hidden pack.
        let mut hidden = words();
        if let Some(c) = hidden.get_mut("nl") {
            c.hidden = true;
        }
        let only = packs::BuildOptions {
            only: vec!["nl".into()],
            ..opts
        };
        assert!(
            packs::build(&m, &hidden, &texts(), &only)
                .unwrap_err()
                .contains("not published")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// The producer's licence file names are the consumer's (craft-libs `font-store`): it refuses a
/// trailing dot, more than 100 characters and Windows device names, so the build must too.
#[test]
fn safe_file_name_matches_the_consumer() {
    for ok in [
        "LICENSE",
        "COPYING.txt",
        "README_nl-NL.md",
        "com0",
        "com10.txt",
        "lpt",
        "console",
    ] {
        assert!(packs::safe_file_name(ok), "{ok} should be accepted");
    }
    let long = "a".repeat(101);
    let longest = "a".repeat(100);
    for bad in [
        "",
        ".hidden",
        "trailing.",
        "bad name",
        "a/b",
        "é.txt",
        long.as_str(),
        "con",
        "CON",
        "Prn.txt",
        "aux.tar.gz",
        "NUL",
        "com1",
        "COM9.md",
        "lpt1",
        "LPT9.txt",
    ] {
        assert!(!packs::safe_file_name(bad), "{bad} should be refused");
    }
    assert!(packs::safe_file_name(&longest));
}
