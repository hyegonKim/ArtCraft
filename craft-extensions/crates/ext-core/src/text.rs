//! Text a person will read (names, descriptions) from an untrusted package.
//!
//! Invariant: control characters, bidi overrides and zero-width characters are refused, because they
//! let a name read differently from what it is. Joiners stay allowed so scripts that need them work.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong,
    ControlCharacter,
    InvisibleFormatting,
    NothingVisible,
    NewlineNotAllowed,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TextError::Empty => "the text is empty",
            TextError::TooLong => "the text is too long",
            TextError::ControlCharacter => "the text contains a control character",
            TextError::InvisibleFormatting => "the text contains invisible formatting characters",
            TextError::NothingVisible => "the text has no visible characters",
            TextError::NewlineNotAllowed => "the text must be on one line",
        })
    }
}

impl std::error::Error for TextError {}

/// Whether line breaks are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lines {
    One,
    Many,
}

/// Characters that change how text is drawn or read without being visible themselves. The joiner
/// and non-joiner (U+200C, U+200D) are deliberately absent: some scripts cannot be written without them.
fn is_invisible_formatting(c: char) -> bool {
    matches!(c,
        '\u{ad}' | '\u{61c}' | '\u{180e}' | '\u{200b}' | '\u{200e}' | '\u{200f}'
        | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

pub fn validate_text(text: &str, max_chars: usize, lines: Lines) -> Result<(), TextError> {
    if text.is_empty() {
        return Err(TextError::Empty);
    }
    // Stop counting as soon as the limit is passed: the input may be enormous.
    if text.chars().take(max_chars.saturating_add(1)).count() > max_chars {
        return Err(TextError::TooLong);
    }
    for c in text.chars() {
        if c == '\n' || c == '\r' {
            if lines == Lines::One {
                return Err(TextError::NewlineNotAllowed);
            }
        } else if c.is_control() {
            return Err(TextError::ControlCharacter);
        } else if is_invisible_formatting(c) {
            return Err(TextError::InvisibleFormatting);
        }
    }
    if text.chars().all(char::is_whitespace) {
        return Err(TextError::NothingVisible);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_text_in_several_scripts() {
        for ok in ["Dark Ocean", "Тёмный океан", "深色海洋", "Café", "نمط داكن"] {
            assert_eq!(validate_text(ok, 64, Lines::One), Ok(()), "{ok}");
        }
    }

    #[test]
    fn refuses_empty_and_whitespace_only() {
        assert_eq!(validate_text("", 64, Lines::One), Err(TextError::Empty));
        assert_eq!(validate_text("   ", 64, Lines::One), Err(TextError::NothingVisible));
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert_eq!(validate_text(&"é".repeat(64), 64, Lines::One), Ok(()));
        assert_eq!(validate_text(&"é".repeat(65), 64, Lines::One), Err(TextError::TooLong));
    }

    #[test]
    fn refuses_control_characters() {
        for bad in ["a\0b", "a\u{7}b", "a\u{1b}[31mb", "a\u{85}b"] {
            assert_eq!(validate_text(bad, 64, Lines::One), Err(TextError::ControlCharacter), "{bad:?}");
        }
    }

    #[test]
    fn refuses_invisible_formatting_but_keeps_joiners() {
        for bad in ["a\u{202e}b", "a\u{200b}b", "a\u{200e}b", "a\u{feff}b", "a\u{ad}b", "a\u{2066}b", "a\u{2060}b", "a\u{61c}b"] {
            assert_eq!(validate_text(bad, 64, Lines::One), Err(TextError::InvisibleFormatting), "{bad:?}");
        }
        assert_eq!(validate_text("क्\u{200d}ष", 64, Lines::One), Ok(()));
        assert_eq!(validate_text("می\u{200c}خواهم", 64, Lines::One), Ok(()));
    }

    #[test]
    fn newlines_only_where_allowed() {
        assert_eq!(validate_text("a\nb", 64, Lines::One), Err(TextError::NewlineNotAllowed));
        assert_eq!(validate_text("a\nb", 64, Lines::Many), Ok(()));
        assert_eq!(validate_text("a\r\nb", 64, Lines::Many), Ok(()));
        assert_eq!(validate_text("a\tb", 64, Lines::Many), Err(TextError::ControlCharacter));
    }
}
