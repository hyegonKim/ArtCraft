//! Locale directory names and how many plural forms a locale's `@plural` rows list.

use craft_i18n::{Embedded, I18n};
use unic_langid::LanguageIdentifier;

/// Ok when `name` is a BCP 47 tag written in lowercase (`pt-br`, `zh-hans`).
///
/// # Errors
///
/// Returns an error when `name` is not a valid BCP 47 tag or is not written in lowercase.
pub fn check_locale_name(name: &str) -> Result<(), String> {
    let id: LanguageIdentifier = name
        .parse()
        .map_err(|e| format!("{name:?} is not a BCP 47 locale: {e}"))?;
    let lower = id.to_string().to_lowercase();
    if lower == name {
        Ok(())
    } else {
        Err(format!("{name:?}: write it as {lower:?}"))
    }
}

/// The most plural forms a CLDR locale has (Arabic: zero, one, two, few, many, other).
const MAX_FORMS: usize = 6;

/// How many plural forms `locale`'s `@plural` rows list, taken from craft-libs' `craft-i18n` runtime
/// crate itself, so the checker passes exactly the rows the apps accept: the CLDR cardinal
/// categories whole numbers 0..=1000 reach, from the crate's own CLDR data. The crate has no call
/// that returns the count, so this embeds one `@plural` row of each count from 1 to [`MAX_FORMS`]
/// and returns the one it doesn't reject.
///
/// # Errors
///
/// Returns an error when the crate has no plural rules for `locale` (not a BCP 47 tag, or a
/// language CLDR has no rules for) or accepts no single count.
pub fn plural_forms(locale: &str) -> Result<usize, String> {
    let catalog: String = (1..=MAX_FORMS)
        .map(|k| {
            let forms = vec!["{n} x"; k].join("|");
            format!("@plural\t{{n}} a{k}|{{n}} b{k}\t{forms}\n")
        })
        .collect();
    // ponytail: `I18n::new` takes `&'static` catalogs, so each call leaks two short strings and
    // one entry; the checker calls this a few times per locale per run.
    let entry: &'static str = Box::leak(locale.to_owned().into_boxed_str());
    let catalog: &'static str = Box::leak(catalog.into_boxed_str());
    let embedded: &'static Embedded = Box::leak(Box::new([(entry, catalog, "")]));
    let (i18n, problems) = I18n::new(embedded, &[]);
    if !problems.is_empty() {
        return Err(format!("{locale:?}: {}", problems.join("; ")));
    }
    let rejected = i18n.check_all();
    let accepted: Vec<usize> = (1..=MAX_FORMS)
        .filter(|k| {
            !rejected
                .iter()
                .any(|p| p.contains(&format!(": line {k}: ")))
        })
        .collect();
    match accepted.as_slice() {
        [n] => Ok(*n),
        _ => Err(format!(
            "{locale:?}: craft-i18n accepts {accepted:?} plural forms: {rejected:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_names() {
        for ok in ["en", "fr", "ko", "pt-br", "zh-hans", "zh-hant", "sr-latn"] {
            assert!(check_locale_name(ok).is_ok(), "{ok}");
        }
        for bad in ["pt_br", "pt-BR", "zh-Hans", "PT", "", "not a locale"] {
            assert!(check_locale_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn plural_form_counts() {
        for (locale, n) in [
            ("fr", 2),
            ("ja", 1),
            ("ko", 1),
            ("vi", 1),
            ("ru", 3),
            ("pl", 3),
            ("cs", 3),
            ("sl", 4),
            ("ar", 6),
            ("pt", 2),
            ("pt-br", 2),
            ("tr", 2),
            ("sr-latn", 3),
            ("zh-hant", 1),
            ("pt-pt", 2),
            ("ckb", 2),
        ] {
            assert_eq!(plural_forms(locale), Ok(n), "{locale}");
        }
        assert!(plural_forms("tlh").is_err(), "no CLDR plural rules");
        assert!(
            plural_forms("en").is_err(),
            "the source language; the crate skips it"
        );
        assert!(plural_forms("not a locale").is_err());
    }
}
