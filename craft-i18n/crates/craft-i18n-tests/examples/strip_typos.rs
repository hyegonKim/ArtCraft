//! One-off: remove entries that are far more often typos than words from the Moby list
//! (`language-data/en-us/en-us.dic`), matching craft-proof's `en_words` generator exclusions.
//!
//! ```sh
//! cargo run -p craft-i18n-tests --example strip_typos -- language-data/en-us/en-us.dic
//! ```
//!
//! Decodes the dictionary (raw deflate), drops the entries below, writes it back re-encoded.
//! Break points of every other entry are kept. Prints what was removed.

use craft_proof::dict::Dictionary;

/// Same list as craft-proof's `EXCLUDED` (`teh` for `the`, `abd`/`nad` for `and`, `ur`, `u`, `r`).
const TYPOS: &[&str] = &["abd", "nad", "r", "teh", "u", "ur"];

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        return Err("usage: strip_typos <en-us.dic>".into());
    };
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let dict = Dictionary::from_bytes(&bytes)?;
    let before = dict.len();
    let kept: Vec<(String, u64)> = dict
        .iter()
        .filter(|(w, _)| {
            let drop = TYPOS.contains(w);
            if drop {
                eprintln!("removing {w:?}");
            }
            !drop
        })
        .map(|(w, m)| (w.to_string(), m))
        .collect();
    let out = Dictionary::from_entries(kept);
    eprintln!("{before} -> {} entries", out.len());
    std::fs::write(path, out.to_bytes()).map_err(|e| format!("{path}: {e}"))?;
    Ok(())
}
