use std::collections::HashSet;

use once_cell::sync::Lazy;

use crate::BANNED_SLURS;
use crate::latin_alphabet::latin_to_ascii;

static BANNED_SLURS_SET : Lazy<HashSet<String>> = Lazy::new(|| {
  BANNED_SLURS.lines()
      .map(|line| line.trim())
      .filter(|line| !(line.starts_with("#") || line.is_empty()))
      .map(|line| line.to_string())
      .collect::<HashSet<String>>()
});

pub fn contains_slurs(unparsed_text: &str) -> bool {
  let simplified = latin_to_ascii(unparsed_text).to_lowercase();
  for wordlike in simplified.split_ascii_whitespace() {
    if BANNED_SLURS_SET.contains(wordlike) {
      return true;
    }
  }
  false
}

#[cfg(test)]
mod tests {
  use crate::check_for_slurs::contains_slurs;

  #[test]
  fn valid_text_passes() {
    assert_eq!(contains_slurs(""), false);
    assert_eq!(contains_slurs("this is a test."), false);
    assert_eq!(contains_slurs("this\nis\na\ntest\n\n"), false);
    assert_eq!(contains_slurs("    this    is    a       test"), false);
  }

  #[test]
  fn text_with_slurs_fails() {
    assert_eq!(contains_slurs("a sentence containing fag is banned"), true);
    assert_eq!(contains_slurs("a\nsentence\ncontaining fags\nis banned"), true);
  }

  #[test]
  fn text_with_mixed_case_slurs_fails() {
    assert_eq!(contains_slurs("FAG"), true);
    assert_eq!(contains_slurs("FaG"), true);
    assert_eq!(contains_slurs("fAg"), true);
    assert_eq!(contains_slurs("A SENTENCE CONTAINING FAG IS BANNED"), true);
    assert_eq!(contains_slurs("a\nsentence\ncontaining FAGS\nis banned"), true);
  }

  #[test]
  fn text_with_latin_obfuscated_slurs_fails() {
    assert_eq!(contains_slurs("FÀG"), true);
    assert_eq!(contains_slurs("FÁG"), true);
    assert_eq!(contains_slurs("FÂG"), true);
    assert_eq!(contains_slurs("FÃG"), true);
    assert_eq!(contains_slurs("FÄG"), true);
    assert_eq!(contains_slurs("FÅG"), true);

    assert_eq!(contains_slurs("fàg"), true);
    assert_eq!(contains_slurs("fág"), true);
    assert_eq!(contains_slurs("fâg"), true);
    assert_eq!(contains_slurs("fãg"), true);
    assert_eq!(contains_slurs("fäg"), true);
    assert_eq!(contains_slurs("fåg"), true);
  }
}
