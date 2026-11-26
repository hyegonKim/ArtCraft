//! `language-packs`: check, fetch and build the Language packs (`docs/language-data-format.md`).
//!
//! ```text
//! language-packs check
//! language-packs fetch [--cache DIR]
//! language-packs build --out DIR --release NAME --version SHA [--base-url URL]
//!                      [--only nl,fr,pl] [--cache DIR] [--no-download]
//! ```
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::path::PathBuf;
use std::process::ExitCode;

use craft_i18n_tests::packs;

const USAGE: &str = "usage:
  language-packs check
  language-packs fetch [--cache DIR]
  language-packs build --out DIR --release NAME --version SHA [--base-url URL]
                       [--only nl,fr,pl] [--cache DIR] [--no-download]

--base-url defaults to https://github.com/storytold/craft-i18n/releases/download/<release>/;
--cache to $CRAFT_I18N_CACHE, else target/language-data-cache.";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("language-packs: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--name value` pairs and bare flags after the subcommand.
struct Args(Vec<String>);

impl Args {
    fn value(&self, name: &str) -> Result<Option<String>, String> {
        match self.0.iter().position(|a| a == name) {
            None => Ok(None),
            Some(i) => self
                .0
                .get(i + 1)
                .filter(|v| !v.starts_with("--"))
                .cloned()
                .map(Some)
                .ok_or_else(|| format!("{name} needs a value")),
        }
    }

    fn required(&self, name: &str) -> Result<String, String> {
        self.value(name)?
            .ok_or_else(|| format!("{name} is required\n{USAGE}"))
    }

    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
}

fn run(argv: Vec<String>) -> Result<(), String> {
    let Some((cmd, rest)) = argv.split_first() else {
        return Err(USAGE.into());
    };
    let args = Args(rest.to_vec());
    let repo = packs::load_repo()?;
    let (manifest, checks, texts) = (&repo.manifest, &repo.checks, &repo.texts);
    let cache = args
        .value("--cache")?
        .map_or_else(packs::default_cache, PathBuf::from);
    match cmd.as_str() {
        "check" => {
            let errors = packs::check(manifest, checks, texts);
            if errors.is_empty() {
                println!(
                    "{} entries, {} locales: ok",
                    manifest.file.len(),
                    manifest.locales().len()
                );
                Ok(())
            } else {
                Err(errors.join("\n"))
            }
        }
        "fetch" => {
            let mut failed = Vec::new();
            for e in &manifest.file {
                match packs::fetch(e, &cache, true) {
                    Ok(_) => println!("ok {} {:?} {}", e.locale, e.kind, e.size),
                    Err(err) => failed.push(err),
                }
            }
            if failed.is_empty() {
                Ok(())
            } else {
                Err(failed.join("\n"))
            }
        }
        "build" => {
            let release = args.required("--release")?;
            let mut base_url = args.value("--base-url")?.unwrap_or_else(|| {
                format!("https://github.com/storytold/craft-i18n/releases/download/{release}/")
            });
            if !base_url.ends_with('/') {
                base_url.push('/');
            }
            let opts = packs::BuildOptions {
                cache,
                out: PathBuf::from(args.required("--out")?),
                release,
                version: args.required("--version")?,
                base_url,
                only: args
                    .value("--only")?
                    .map(|s| s.split(',').map(str::to_owned).collect())
                    .unwrap_or_default(),
                download: !args.flag("--no-download"),
            };
            let errors = packs::check(manifest, checks, texts);
            if !errors.is_empty() {
                return Err(errors.join("\n"));
            }
            let index = packs::build(manifest, checks, texts, &opts)?;
            for p in &index.packs {
                println!("{} {} bytes {}", p.locale, p.size, p.class);
            }
            println!(
                "{} packs and index.json in {}",
                index.packs.len(),
                opts.out.display()
            );
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
