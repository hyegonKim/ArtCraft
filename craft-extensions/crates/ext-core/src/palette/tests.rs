use super::*;
use serde_json::{Value, json};

fn dark() -> Value {
    json!({
        "schema": 1,
        "palette": {
            "dark": true, "background": "#0f1720", "surface": "#18222e", "text": "#e8eef4", "muted_text": "#a9b7c6",
            "line": "#2b3a4a", "accent": "#4da3ff", "good": "#4cc38a", "warn": "#e0b04a", "bad": "#ff6b6b"
        }
    })
}

fn light() -> Value {
    json!({
        "schema": 1,
        "palette": {
            "dark": false, "background": "#ffffff", "surface": "#f1f3f5", "text": "#14181d", "muted_text": "#4a5561",
            "line": "#c9d0d8", "accent": "#0b5cc2", "good": "#1b7f4d", "warn": "#8a5a00", "bad": "#b3261e"
        }
    })
}

fn parse(v: &Value) -> Result<ThemeFile, ThemeError> {
    ThemeFile::parse(v.to_string().as_bytes())
}

#[test]
fn colours_parse_strictly() {
    assert_eq!(Rgb::parse("#0a1B2c"), Some(Rgb(10, 27, 44)));
    assert_eq!(Rgb::parse("#000000"), Some(Rgb(0, 0, 0)));
    for bad in ["", "#fff", "0a1b2c", "#0a1b2", "#0a1b2c3", "#0a1b2cff", "#0a1b2g", "# 0a1b2c", "#0a1b2c ", "rgb(1,2,3)", "#ééé"] {
        assert_eq!(Rgb::parse(bad), None, "{bad:?}");
    }
    assert_eq!(Rgb(10, 27, 44).to_string(), "#0a1b2c");
}

#[test]
fn contrast_matches_the_wcag_definition() {
    let black = Rgb(0, 0, 0);
    let white = Rgb(255, 255, 255);
    assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
    assert!((contrast_ratio(white, black) - 21.0).abs() < 0.01, "order does not matter");
    assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-9);
    // The classic boundary: #767676 on white is the darkest grey that passes 4.5:1.
    assert!(contrast_ratio(Rgb(0x76, 0x76, 0x76), white) >= 4.5);
    assert!(contrast_ratio(Rgb(0x77, 0x77, 0x77), white) < 4.5);
    assert!((white.luminance() - 1.0).abs() < 1e-9 && black.luminance().abs() < 1e-9);
}

#[test]
fn a_dark_and_a_light_theme_parse() {
    let d = parse(&dark()).unwrap();
    assert!(d.palette.dark && d.apps.is_empty());
    let l = parse(&light()).unwrap();
    assert!(!l.palette.dark);
}

#[test]
fn low_contrast_names_the_pair() {
    let cases = [
        ("text", "#101820", Pair::TextOnBackground),
        ("accent", "#0f1720", Pair::AccentOnBackground),
        ("good", "#101820", Pair::GoodOnBackground),
        ("warn", "#101820", Pair::WarnOnBackground),
        ("bad", "#101820", Pair::BadOnBackground),
    ];
    for (field, colour, pair) in cases {
        let mut v = dark();
        v["palette"][field] = json!(colour);
        assert_eq!(parse(&v), Err(ThemeError::LowContrast(pair)), "{field}");
    }
    // Text that passes on the background but not on the panel colour.
    let mut v = dark();
    v["palette"]["surface"] = json!("#9aa8b6");
    v["palette"]["muted_text"] = json!("#a9b7c6");
    assert_eq!(parse(&v), Err(ThemeError::LowContrast(Pair::TextOnSurface)));
}

#[test]
fn secondary_text_must_clear_the_background_too() {
    let mut v = dark();
    v["palette"]["muted_text"] = json!("#1a2430");
    assert_eq!(parse(&v), Err(ThemeError::LowContrast(Pair::MutedTextOnBackground)));
}

#[test]
fn secondary_text_must_clear_the_panel_colour_too() {
    // Passes on a black background (9.7:1) but not on a mid-grey panel (3.2:1).
    let mut v = dark();
    v["palette"]["background"] = json!("#000000");
    v["palette"]["surface"] = json!("#595959");
    v["palette"]["text"] = json!("#ffffff");
    v["palette"]["muted_text"] = json!("#b0b0b0");
    assert_eq!(parse(&v), Err(ThemeError::LowContrast(Pair::MutedTextOnSurface)));
}

#[test]
fn the_dark_flag_must_match_the_colours() {
    let mut v = dark();
    v["palette"]["dark"] = json!(false);
    assert_eq!(parse(&v), Err(ThemeError::InconsistentMode));
    let mut v = light();
    v["palette"]["dark"] = json!(true);
    assert_eq!(parse(&v), Err(ThemeError::InconsistentMode));
}

#[test]
fn schema_size_and_shape() {
    let mut v = dark();
    v["schema"] = json!(2);
    assert_eq!(parse(&v), Err(ThemeError::UnsupportedSchema(2)));
    assert_eq!(ThemeFile::parse(&vec![b' '; MAX_THEME_BYTES + 1]), Err(ThemeError::TooLarge));
    assert_eq!(ThemeFile::parse(b"nope"), Err(ThemeError::NotReadable));
    let mut v = dark();
    v["palette"]["text"] = json!("white");
    assert_eq!(parse(&v), Err(ThemeError::NotReadable));
    let mut v = dark();
    v["palette"].as_object_mut().unwrap().remove("bad");
    assert_eq!(parse(&v), Err(ThemeError::NotReadable));
    let mut v = dark();
    v["future_field"] = json!([1, 2, 3]);
    assert!(parse(&v).is_ok(), "unknown fields are ignored");
}

#[test]
fn per_app_overrides_are_kept_and_bounded() {
    let mut v = dark();
    v["apps"] = json!({"photocraft": {"chrome": "#111111", "canvas_grid": "#222222"}, "launcher": {"bg": "#000000"}});
    let t = parse(&v).unwrap();
    let app = AppId::new("photocraft").unwrap();
    assert_eq!(t.overrides_for(&app).unwrap().get("chrome"), Some(&Rgb(17, 17, 17)));
    assert!(t.overrides_for(&AppId::new("filmcraft").unwrap()).is_none());

    let mut v = dark();
    let mut tokens = serde_json::Map::new();
    for i in 0..=MAX_TOKENS_PER_APP {
        tokens.insert(format!("t{i}"), json!("#123456"));
    }
    v["apps"] = json!({"photocraft": Value::Object(tokens)});
    assert_eq!(parse(&v), Err(ThemeError::TooManyTokens));

    let mut v = dark();
    let mut apps = serde_json::Map::new();
    for i in 0..=MAX_OVERRIDE_APPS {
        apps.insert(format!("app{}{}", (b'a' + (i % 26) as u8) as char, "x".repeat(i / 26)), json!({"a": "#123456"}));
    }
    v["apps"] = Value::Object(apps);
    assert_eq!(parse(&v), Err(ThemeError::TooManyApps));

    for bad in ["", "Chrome", "1chrome", "chro me", "chrome-bg", "é", &"a".repeat(33)] {
        let mut v = dark();
        v["apps"] = json!({"photocraft": {bad: "#123456"}});
        assert_eq!(parse(&v), Err(ThemeError::BadTokenName), "{bad:?}");
    }
    let mut v = dark();
    v["apps"] = json!({"Photo Craft": {"a": "#123456"}});
    assert_eq!(parse(&v), Err(ThemeError::NotReadable));
}

#[test]
fn a_parsed_theme_round_trips_with_lowercase_colours() {
    let mut v = dark();
    v["palette"]["accent"] = json!("#4DA3FF");
    let t = parse(&v).unwrap();
    let out = serde_json::to_string(&t).unwrap();
    assert!(out.contains("#4da3ff") && !out.contains("#4DA3FF"));
    assert_eq!(ThemeFile::parse(out.as_bytes()).unwrap(), t);
}

#[test]
fn messages_are_plain_language() {
    let all = [
        ThemeError::TooLarge,
        ThemeError::NotReadable,
        ThemeError::UnsupportedSchema(2),
        ThemeError::LowContrast(Pair::TextOnBackground),
        ThemeError::LowContrast(Pair::BadOnBackground),
        ThemeError::InconsistentMode,
        ThemeError::TooManyApps,
        ThemeError::TooManyTokens,
        ThemeError::BadTokenName,
    ];
    for e in all {
        let text = e.to_string().to_lowercase();
        for word in ["wcag", "rgb", "hex", "token", "schema", "json", "palette", "luminance"] {
            assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == word), "{e:?} says {word}: {text}");
        }
    }
}
