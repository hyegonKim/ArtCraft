//! The checker and craft-libs' `craft-i18n` runtime crate agree on how many plural forms each
//! locale in `locales/` takes. They disagreeing means a catalog CI passes has `@plural` rows the
//! apps skip, or the reverse. The checker takes the count from the crate (`locale::plural_forms`);
//! these check it from two other directions.

use craft_i18n::{Embedded, I18n};
use craft_i18n_tests::locale::plural_forms;

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Every locale directory in `locales/`.
fn locales() -> Vec<String> {
    let dir = craft_i18n_tests::repo_root().join("locales");
    let mut locales: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    locales.sort();
    assert!(locales.len() > 40, "{locales:?}");
    locales
}

/// The runtime crate's problems with one `@plural` row of `forms` forms for `locale`.
fn runtime_problems(locale: &str, forms: usize) -> Vec<String> {
    let translation = vec!["{n} x"; forms].join("|");
    let catalog = leak(format!("@plural\t{{n}} a|{{n}} b\t{translation}\n"));
    let embedded: &'static Embedded = Box::leak(Box::new([(leak(locale.to_owned()), catalog, "")]));
    let (i18n, mut problems) = I18n::new(embedded, &[]);
    problems.extend(i18n.check_all());
    problems
}

#[test]
fn plural_form_counts_match_the_runtime_crate() {
    let mut disagree = Vec::new();
    for locale in locales() {
        let Ok(n) = plural_forms(&locale) else {
            disagree.push(format!("{locale}: {:?}", plural_forms(&locale)));
            continue;
        };
        // The crate takes exactly `n` forms: `n` passes, one fewer and one more don't.
        let ok = runtime_problems(&locale, n);
        let fewer = n > 1 && runtime_problems(&locale, n - 1).is_empty();
        let more = runtime_problems(&locale, n + 1).is_empty();
        if !ok.is_empty() || fewer || more {
            disagree.push(format!("{locale}: checker {n}, runtime {ok:?}"));
        }
    }
    assert_eq!(disagree, Vec::<String>::new());
}

/// The crate, given the repository's own catalogs (`common.tsv` with each app's), skips no
/// `@plural` row the checker passed.
#[test]
fn the_runtime_crate_accepts_every_plural_row() {
    let root = craft_i18n_tests::repo_root().join("locales");
    let mut problems = Vec::new();
    for locale in locales() {
        let dir = root.join(&locale);
        let common = leak(std::fs::read_to_string(dir.join("common.tsv")).unwrap_or_default());
        let mut apps: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "tsv") && !p.ends_with("common.tsv"))
            .collect();
        apps.sort();
        for app in apps {
            let text = leak(std::fs::read_to_string(&app).unwrap());
            let embedded: &'static Embedded =
                Box::leak(Box::new([(leak(locale.clone()), common, text)]));
            let (i18n, new_problems) = I18n::new(embedded, &[]);
            problems.extend(new_problems);
            problems.extend(
                i18n.check_all()
                    .into_iter()
                    .filter(|p| p.contains("plural"))
                    .map(|p| format!("{}: {p}", app.display())),
            );
        }
    }
    assert_eq!(problems, Vec::<String>::new());
}

/// The comparison itself catches a wrong count.
#[test]
fn a_wrong_count_is_caught() {
    assert!(runtime_problems("fr", 2).is_empty());
    assert!(!runtime_problems("fr", 3).is_empty());
    assert!(!runtime_problems("ru", 2).is_empty());
    assert!(!runtime_problems("ckb", 1).is_empty());
    assert!(
        !runtime_problems("tlh", 1).is_empty(),
        "no CLDR plural rules"
    );
}

/// `tools/plural_forms.tsv`, the gettext `Plural-Forms` the Weblate converter writes, picks the
/// same form as the runtime crate for every whole number 0..=1000 in every locale: a wrong order
/// there would show "1 page" wording for 5 pages once a Weblate translation is imported.
#[test]
fn plural_forms_table_matches_the_runtime_crate() {
    let path = craft_i18n_tests::repo_root().join("tools/plural_forms.tsv");
    let table = std::fs::read_to_string(path).unwrap();
    let rows: Vec<(&str, usize, &str)> = table
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            assert_eq!(cols.len(), 3, "{l:?}");
            (cols[0], cols[1].parse().unwrap(), cols[2])
        })
        .collect();
    let listed: Vec<String> = rows.iter().map(|r| r.0.to_owned()).collect();
    assert_eq!(listed, locales(), "one row per locale directory, sorted");

    let mut disagree = Vec::new();
    for (locale, nplurals, expr) in rows {
        if plural_forms(locale) != Ok(nplurals) {
            disagree.push(format!(
                "{locale}: nplurals {nplurals}, checker {:?}",
                plural_forms(locale)
            ));
            continue;
        }
        let forms: Vec<String> = (0..nplurals).map(|i| format!("{{n}}#{i}")).collect();
        let catalog = leak(format!("@plural\t{{n}} a|{{n}} b\t{}\n", forms.join("|")));
        let embedded: &'static Embedded =
            Box::leak(Box::new([(leak(locale.to_owned()), catalog, "")]));
        let (i18n, problems) = I18n::new(embedded, &[locale]);
        assert_eq!(problems, Vec::<String>::new(), "{locale}");
        let lang = i18n.from_code(locale).unwrap();
        if let Some(n) = (0..=1000u64).find(|&n| {
            let shown = i18n.trn_plain(lang, n, "{n} a", "{n} b");
            let runtime: u64 = shown.rsplit('#').next().unwrap().parse().unwrap();
            eval_plural(expr, n) != runtime
        }) {
            disagree.push(format!(
                "{locale}: {expr:?} picks form {} for {n}, the runtime crate {:?}",
                eval_plural(expr, n),
                i18n.trn_plain(lang, n, "{n} a", "{n} b")
            ));
        }
    }
    assert_eq!(disagree, Vec::<String>::new());
}

/// A gettext plural expression (C syntax, in `n`) for one `n`: the grammar `Plural-Forms` uses.
fn eval_plural(expr: &str, n: u64) -> u64 {
    let tokens = tokenize(expr);
    let mut p = Plural {
        tokens: &tokens,
        at: 0,
        n,
    };
    let value = p.ternary();
    assert_eq!(p.at, tokens.len(), "trailing tokens in {expr:?}");
    value
}

#[derive(Debug, PartialEq)]
enum Token {
    Num(u64),
    N,
    Op(&'static str),
}

fn tokenize(mut rest: &str) -> Vec<Token> {
    // Two-character operators first, so `<=` isn't read as `<`.
    const OPS: [&str; 14] = [
        "==", "!=", "<=", ">=", "&&", "||", "<", ">", "%", "?", ":", "(", ")", "!",
    ];
    let mut out = Vec::new();
    rest = rest.trim_start();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('n') {
            out.push(Token::N);
            rest = r;
        } else if rest.starts_with(|c: char| c.is_ascii_digit()) {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            out.push(Token::Num(rest[..end].parse().unwrap()));
            rest = &rest[end..];
        } else {
            let op = OPS
                .iter()
                .find(|o| rest.starts_with(**o))
                .unwrap_or_else(|| panic!("bad token at {rest:?}"));
            out.push(Token::Op(op));
            rest = &rest[op.len()..];
        }
        rest = rest.trim_start();
    }
    out
}

struct Plural<'a> {
    tokens: &'a [Token],
    at: usize,
    n: u64,
}

impl Plural<'_> {
    fn eat(&mut self, op: &str) -> bool {
        let hit = matches!(self.tokens.get(self.at), Some(Token::Op(o)) if *o == op);
        self.at += usize::from(hit);
        hit
    }

    fn ternary(&mut self) -> u64 {
        let cond = self.binary(0);
        if !self.eat("?") {
            return cond;
        }
        let yes = self.ternary();
        assert!(self.eat(":"), "`?` without `:`");
        let no = self.ternary();
        if cond != 0 { yes } else { no }
    }

    /// Binary operators by precedence level, loosest first.
    fn binary(&mut self, level: usize) -> u64 {
        const LEVELS: [&[&str]; 5] = [
            &["||"],
            &["&&"],
            &["==", "!="],
            &["<=", ">=", "<", ">"],
            &["%"],
        ];
        let Some(ops) = LEVELS.get(level) else {
            return self.unary();
        };
        let mut left = self.binary(level + 1);
        while let Some(&op) = ops.iter().find(|op| self.eat(op)) {
            let right = self.binary(level + 1);
            left = match op {
                "||" => u64::from(left != 0 || right != 0),
                "&&" => u64::from(left != 0 && right != 0),
                "==" => u64::from(left == right),
                "!=" => u64::from(left != right),
                "<=" => u64::from(left <= right),
                ">=" => u64::from(left >= right),
                "<" => u64::from(left < right),
                ">" => u64::from(left > right),
                _ => left % right,
            };
        }
        left
    }

    fn unary(&mut self) -> u64 {
        if self.eat("!") {
            return u64::from(self.unary() == 0);
        }
        if self.eat("(") {
            let value = self.ternary();
            assert!(self.eat(")"), "unclosed `(`");
            return value;
        }
        let value = match self.tokens.get(self.at) {
            Some(Token::Num(v)) => *v,
            Some(Token::N) => self.n,
            other => panic!("expected a number or n, got {other:?}"),
        };
        self.at += 1;
        value
    }
}

#[test]
fn eval_plural_follows_c_precedence() {
    assert_eq!(eval_plural("n != 1", 1), 0);
    assert_eq!(eval_plural("n%10==1 && n%100!=11 ? 0 : 1", 111), 1);
    assert_eq!(eval_plural("n==0 ? 0 : n==1 ? 1 : 2", 7), 2);
    assert_eq!(eval_plural("!(n > 1)", 0), 1);
}
