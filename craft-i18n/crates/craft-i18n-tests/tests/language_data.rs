//! `language-data/`: every file has exactly one row in `ATTRIBUTION.md`, with a licence and an
//! Embeddable Licence class (ADR 0004). Committed files are embedded; Language packs are checked
//! from the manifest (`tests/packs.rs`).

use std::path::Path;

/// Licences that are Download-only, so never in a committed (embedded) file (ADR 0004, signed off
/// 2026-11-01; MPL and LPPL are Embeddable unmodified). `GPL` also matches LGPL and AGPL.
/// ponytail: substring deny-list on a prose cell; the manifest uses SPDX allow-lists.
const DOWNLOAD_ONLY: [&str; 2] = ["GPL", "CC-BY-SA"];

/// Every file under `dir`, dotfiles included, as a path relative to `root` with `/` separators.
fn files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let list = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in list {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).unwrap();
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(parts.join("/"));
        }
    }
}

/// Files under `language-data/` that aren't upstream data (`tests/packs.rs` checks them): the
/// README, the Language pack manifest and check words (this repository's licence), and the
/// standard licence texts packs carry (`licences/<SPDX id>.txt`, from spdx/license-list-data; each
/// licence permits verbatim copies of itself).
fn ours(path: &str) -> bool {
    path == "language-data/README.md"
        || path == "language-data/manifest.toml"
        || (path.starts_with("language-data/checks/") && path.ends_with(".toml"))
        || (path.starts_with("language-data/licences/") && path.ends_with(".txt"))
}

/// The problems with `files` (paths under `language-data/`) against `attribution`'s rows.
fn check(files: &[String], attribution: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let rows: Vec<(&str, &str, &str)> = attribution
        .lines()
        .filter(|l| l.starts_with("| `language-data/"))
        .map(|line| {
            let cells: Vec<&str> = line.split(" | ").collect();
            let path = line
                .trim_start_matches("| `")
                .split('`')
                .next()
                .unwrap_or("");
            (
                path,
                cells.get(3).map_or("", |c| c.trim()),
                cells.get(4).map_or("", |c| c.trim()),
            )
        })
        .collect();
    for (path, licence, class) in &rows {
        if !files.iter().any(|f| f == path) {
            errors.push(format!("{path}: row without a file"));
        }
        if licence.is_empty() {
            errors.push(format!("{path}: no licence"));
        }
        // Only the licence used, not the alternatives the notes in brackets mention.
        let used = licence.split(" (").next().unwrap_or("");
        if let Some(g) = DOWNLOAD_ONLY.iter().find(|g| used.contains(*g)) {
            errors.push(format!(
                "{path}: {g} is Download-only and can't be committed (ADR 0004)"
            ));
        }
        if *class != "Embeddable" {
            errors.push(format!(
                "{path}: Licence class is {class:?}; only Embeddable ships until Language packs do"
            ));
        }
    }
    for f in files.iter().filter(|f| !ours(f)) {
        match rows.iter().filter(|r| r.0 == f).count() {
            0 => errors.push(format!("{f}: no ATTRIBUTION.md row")),
            1 => {}
            n => errors.push(format!("{f}: {n} ATTRIBUTION.md rows")),
        }
    }
    errors
}

#[test]
fn language_data_is_attributed_and_embeddable() {
    let root = craft_i18n_tests::repo_root();
    let attribution = std::fs::read_to_string(root.join("ATTRIBUTION.md")).unwrap();
    let mut all = Vec::new();
    files(&root, &root.join("language-data"), &mut all);
    assert!(all.len() > 1, "language-data/ is empty");
    let errors = check(&all, &attribution);
    assert!(errors.is_empty(), "language-data:\n{}", errors.join("\n"));
}

#[cfg(test)]
mod fixtures {
    use super::check;

    const HEAD: &str =
        "| File | Author | Source | Licence | Licence class | Notes |\n|---|---|---|---|---|---|\n";

    fn run(files: &[&str], rows: &[&str]) -> Vec<String> {
        let files: Vec<String> = files.iter().map(|f| (*f).to_owned()).collect();
        check(&files, &format!("{HEAD}{}\n", rows.join("\n")))
    }

    fn row(path: &str, licence: &str, class: &str) -> String {
        format!("| `{path}` | someone | somewhere | {licence} | {class} | notes |")
    }

    #[test]
    fn clean() {
        let r = row(
            "language-data/de/hyphenation.pat",
            "MIT (offered under LPPL or MIT)",
            "Embeddable",
        );
        assert!(
            run(
                &[
                    "language-data/README.md",
                    "language-data/manifest.toml",
                    "language-data/checks/nl.toml",
                    "language-data/de/hyphenation.pat"
                ],
                &[&r]
            )
            .is_empty()
        );
    }

    /// Signed off (ADR 0004, 2026-11-01): MPL and LPPL files are Embeddable when unmodified.
    #[test]
    fn mpl_and_lppl_are_embeddable() {
        for licence in ["MPL-2.0", "LPPL-1.3c"] {
            let r = row("language-data/fr/x.dic", licence, "Embeddable");
            assert!(
                run(&["language-data/fr/x.dic"], &[&r]).is_empty(),
                "{licence}"
            );
        }
    }

    #[test]
    fn each_failure() {
        let cases = [
            (
                vec!["language-data/de/.LICENSE"],
                vec![],
                "no ATTRIBUTION.md row",
            ),
            (
                vec![],
                vec![row("language-data/fr/x.dic", "MIT", "Embeddable")],
                "row without a file",
            ),
            (
                vec!["language-data/fr/x.dic"],
                vec![row("language-data/fr/x.dic", "", "Embeddable")],
                "no licence",
            ),
            (
                vec!["language-data/fr/x.dic"],
                vec![row("language-data/fr/x.dic", "LGPL-2.1-only", "Embeddable")],
                "GPL is Download-only",
            ),
            (
                vec!["language-data/fr/x.dic"],
                vec![row("language-data/fr/x.dic", "CC-BY-SA-4.0", "Embeddable")],
                "CC-BY-SA is Download-only",
            ),
            (
                vec!["language-data/fr/x.dic"],
                vec![row("language-data/fr/x.dic", "MIT", "Download-only")],
                "Licence class is",
            ),
            (
                vec!["language-data/fr/x.dic"],
                vec![row("language-data/fr/x.dic", "MIT", "Excluded")],
                "Licence class is",
            ),
            (
                vec!["language-data/fr/x.dic"],
                vec![
                    row("language-data/fr/x.dic", "MIT", "Embeddable"),
                    row("language-data/fr/x.dic", "MIT", "Embeddable"),
                ],
                "2 ATTRIBUTION.md rows",
            ),
        ];
        for (files, rows, want) in cases {
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            let errors = run(&files, &rows);
            assert!(
                errors.iter().any(|e| e.contains(want)),
                "{want}: got {errors:?}"
            );
        }
    }
}

/// The Moby word list leaves out entries that are far more often typos than words (the same
/// exclusions as craft-proof's generator), and keeps ordinary neighbours.
#[test]
fn moby_list_has_no_common_typos() {
    let bytes =
        std::fs::read(craft_i18n_tests::repo_root().join("language-data/en-us/en-us.dic")).unwrap();
    let dict = craft_proof::dict::Dictionary::from_bytes(&bytes).unwrap();
    let words: std::collections::HashSet<&str> = dict.iter().map(|(w, _)| w).collect();
    for typo in ["teh", "abd", "nad", "ur", "u", "r"] {
        assert!(!words.contains(typo), "{typo} is a typo, not a word");
    }
    for word in ["the", "and", "your", "tea", "ten"] {
        assert!(words.contains(word), "{word} should still be a word");
    }
}
