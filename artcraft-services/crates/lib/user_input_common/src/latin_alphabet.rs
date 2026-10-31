use std::collections::HashMap;

use once_cell::sync::Lazy;
use unicode_segmentation::UnicodeSegmentation;

fn to_owned(item: &(&str, &str)) -> (String, String) {
  (item.0.to_string(), item.1.to_string())
}

// http://www.geocities.ws/click2speak/unicode/chars_es.html
pub static LATIN_TO_ASCII_CHARACTER_MAP : Lazy<HashMap<String, String>> = Lazy::new(|| {
  // NB: Note that certain latin characters map to multiple ascii characters, eg. "AE".
  let table = [
    ("\u{AA}", "a"), // Feminine ordinal
    ("\u{BA}", "o"), // Masculine ordinal
    ("\u{C0}", "A"), // Latin capital letter A with grave
    ("\u{C1}", "A"), // Latin capital letter A with acute
    ("\u{C2}", "A"), // Latin capital letter A with circumflex
    ("\u{C3}", "A"), // Latin capital letter A with tilde
    ("\u{C4}", "A"), // Latin capital letter A diaeresis
    ("\u{C5}", "A"), // Latin capital letter A with ring above
    ("\u{C6}", "AE"), // Latin capital letter AE
    ("\u{C7}", "C"), // Latin capital letter C with cedilla
    ("\u{C8}", "E"), // Latin capital letter E with grave
    ("\u{C9}", "E"), // Latin capital letter E with acute
    ("\u{CA}", "E"), // Latin capital letter E with circumflex
    ("\u{CB}", "E"), // Latin capital letter E with diaeresis
    ("\u{CC}", "I"), // Latin capital letter I with grave
    ("\u{CD}", "I"), // Latin capital letter I with acute
    ("\u{CE}", "I"), // Latin capital letter I with circumflex
    ("\u{CF}", "I"), // Latin capital letter I with diaeresis
    ("\u{D0}", "D"), // Latin capital letter ETH (~D) TODO: How to handle this?
    ("\u{D1}", "N"), // Latin capital letter N with tilde
    ("\u{D2}", "O"), // Latin capital letter O with grave
    ("\u{D3}", "O"), // Latin capital letter O with acute
    ("\u{D4}", "O"), // Latin capital letter O with circumflex
    ("\u{D5}", "O"), // Latin capital letter O with tilde
    ("\u{D6}", "O"), // Latin capital letter O with diaeresis
    ("\u{D8}", "O"), // Latin capital letter O with stroke (NB: Skips D7 multiplication sign)
    ("\u{D9}", "U"), // Latin capital letter U with grave
    ("\u{DA}", "U"), // Latin capital letter U with acute
    ("\u{DB}", "U"), // Latin capital letter U with circumflex
    ("\u{DC}", "U"), // Latin capital letter U with diaeresis
    ("\u{DD}", "Y"), // Latin capital letter Y with acute
    ("\u{DE}", "P"), // Latin capital letter THORN (~P) TODO: How to handle this?
    ("\u{DF}", "b"), // Latin small letter sharp s (~B) TODO: How to handle this?
    ("\u{E0}", "a"), // Latin small letter a with grave
    ("\u{E1}", "a"), // Latin small letter a with acute
    ("\u{E2}", "a"), // Latin small letter a with circumflex
    ("\u{E3}", "a"), // Latin small letter a with tilde
    ("\u{E4}", "a"), // Latin small letter a with diaeresis
    ("\u{E5}", "a"), // Latin small letter a with ring above
    ("\u{E6}", "ae"), // Latin small letter ae
    ("\u{E7}", "c"), // Latin small letter c with cedilla
    ("\u{E8}", "e"), // Latin small letter e with grave
    ("\u{E9}", "e"), // Latin small letter e with acute
    ("\u{EA}", "e"), // Latin small letter e with circumflex
    ("\u{EB}", "e"), // Latin small letter e with diaeresis
    ("\u{EC}", "i"), // Latin small letter i with grave
    ("\u{ED}", "i"), // Latin small letter i with acute
    ("\u{EE}", "i"), // Latin small letter i with circumflex
    ("\u{EF}", "i"), // Latin small letter i with diaeresis
    ("\u{F0}", "d"), // Latin capital letter eth (~d) TODO: How to handle this?
    ("\u{F1}", "n"), // Lower n tilde
    ("\u{F2}", "o"), // Latin small letter o with grave
    ("\u{F3}", "o"), // Latin small letter o with acute
    ("\u{F4}", "o"), // Latin small letter o with circumflex
    ("\u{F5}", "o"), // Latin small letter o with tilde
    ("\u{F6}", "o"), // Latin small letter o with diaeresis
    ("\u{F8}", "o"), // Latin small letter o with stroke (NB: Skips F7 division sign)
    ("\u{F9}", "u"), // Latin small letter u with grave
    ("\u{FA}", "u"), // Latin small letter u with acute
    ("\u{FB}", "u"), // Latin small letter u with circumflex
    ("\u{FC}", "u"), // Latin small letter u with diaeresis
    ("\u{FD}", "y"), // Latin small letter y with acute
    ("\u{FE}", "p"), // Latin small letter thorn (~p) TODO: How to handle this?
    ("\u{FF}", "y"), // Latin small letter y with diaeresis

    // TODO: There are a lot more to go...
  ];

  let mut map = HashMap::new();

  map.extend(table.iter().map(|item| (item.0.to_string(), item.1.to_string())));

  // Latin Extended-A
  // https://unicode-table.com/en/blocks/latin-extended-a/
  map.extend([
    ("\u{0100}", "A"), // Ā Latin Capital Letter a with Macron
    ("\u{0101}", "a"), // ā Latin Small Letter a with Macron
    ("\u{0102}", "A"), // Ă Latin Capital Letter a with Breve
    ("\u{0103}", "a"), // ă Latin Small Letter a with Breve
    ("\u{0104}", "A"), // Ą Latin Capital Letter a with Ogonek
    ("\u{0105}", "a"), // ą Latin Small Letter a with Ogonek
    ("\u{0106}", "C"), // Ć Latin Capital Letter C with Acute
    ("\u{0107}", "c"), // ć Latin Small Letter C with Acute
    ("\u{0108}", "C"), // Ĉ Latin Capital Letter C with Circumflex
    ("\u{0109}", "c"), // ĉ Latin Small Letter C with Circumflex
    ("\u{010A}", "C"), // Ċ Latin Capital Letter C with Dot Above
    ("\u{010B}", "c"), // ċ Latin Small Letter C with Dot Above
    ("\u{010C}", "C"), // Č Latin Capital Letter C with Caron
    ("\u{010D}", "c"), // č Latin Small Letter C with Caron
    ("\u{010E}", "D"), // Ď Latin Capital Letter D with Caron
    ("\u{010F}", "d"), // ď Latin Small Letter D with Caron
    // TODO: Not done...
    ("\u{0113}", "e"), // ē Latin Small Letter E with Macron
    ("\u{015B}", "s"), // ś Latin Small Letter S with Acute
    ("\u{0161}", "s"), // š Latin Small Letter S with Caron
    ("\u{017C}", "z"), // ż Latin Small Letter Z with Dot Above
    // TODO: Incomplete
  ].iter().map(&to_owned));

  // Latin Extended-B
  // https://unicode-table.com/en/blocks/latin-extended-b/
  map.extend([
    ("\u{01B0}", "u"), // ư Latin Small Letter U with Horn
    ("\u{01B1}", "U"), // Ʊ Latin Capital Letter Upsilon (TODO: Not correct handling)
    ("\u{01B3}", "Y"), // Ƴ Latin Capital Letter Y with Hook
    ("\u{01B4}", "y"), // ƴ Latin Small Letter Y with Hook
    ("\u{01B5}", "Z"), // Ƶ Latin Capital Letter Z with Stroke
    // TODO: Incomplete...
    ("\u{01D0}", "i"), // ǐ Latin Small Letter I with Caron
    ("\u{01D1}", "O"), // Ǒ Latin Capital Letter O with Caron
    ("\u{01D2}", "o"), // ǒ Latin Small Letter O with Caron
    ("\u{01D3}", "U"), // Ǔ Latin Capital Letter U with Caron
    ("\u{01D4}", "u"), // ǔ Latin Small Letter U with Caron
  ].iter().map(&to_owned));

  // Greek and Coptic
  // https://unicode-table.com/en/blocks/greek-coptic/
  map.extend([
    ("\u{03BD}", "v"), // ν Greek Small Letter Nu (TODO: Incorrect handling, but very frequent.)
    ("\u{03BF}", "o"), // ο Greek Small Letter Omicron (TODO: Incorrect handling, but very frequent.)
    // TODO: Incomplete
  ].iter().map(&to_owned));

  // Latin Extended Additional
  // https://unicode-table.com/en/blocks/latin-extended-additional/
  map.extend([
    ("\u{1EA1}", "a"), // ạ Latin Small Letter a with Dot Below
    ("\u{1EB9}", "e"), // ẹ Latin Small Letter E with Dot Below
    // TODO: Incomplete
  ].iter().map(&to_owned));

  // Misc characters that frequently occur
  map.insert("\u{00F3}".to_string(), "o".to_string());  // Latin Small Letter O with Acute
  map.insert("\u{0131}".to_string(), "i".to_string());  // Latin Small Letter Dotless I
  map.insert("\u{00FC}".to_string(), "u".to_string());  // Latin Small Letter U with Diaeresis
  map.insert("\u{015F}".to_string(), "s".to_string());  // Latin Small Letter S with Cedilla
  map.insert("\u{00F6}".to_string(), "o".to_string());  // Latin Small Letter O with Diaeresis
  map.insert("\u{011F}".to_string(), "g".to_string());  // Latin Small Letter G with Breve
  map.insert("\u{0130}".to_string(), "I".to_string());  // Latin Capital Letter I with Dot Above
  map.insert("\u{0259}".to_string(), "e".to_string());  // Latin Small Letter Schwa (TODO: Inaccurate?)
  map.insert("\u{012B}".to_string(), "i".to_string());  // Latin Small Letter I with Macron
  map.insert("\u{1D3A}".to_string(), "n".to_string());  // Modifier Letter Capital N
  map.insert("\u{01D0}".to_string(), "i".to_string());  // Latin Small Letter I with Caron
  map.insert("\u{014D}".to_string(), "o".to_string());  // Latin Small Letter O with Macron
  map.insert("\u{0101}".to_string(), "a".to_string());  // Latin Small Letter A with Macron
  map.insert("\u{00F9}".to_string(), "u".to_string());  // Latin Small Letter U with Grave
  map.insert("\u{01Ce}".to_string(), "a".to_string());  // Latin Small Letter A with Caron
  map
});

pub fn latin_to_ascii(input_text: &str) -> String {
  let segmented = UnicodeSegmentation::graphemes(input_text, true)
      .map(|segment| {
        LATIN_TO_ASCII_CHARACTER_MAP.get(segment)
            .map(|replace| replace.as_str())
            .unwrap_or(segment)
      })
      .collect::<Vec<&str>>();

  segmented.join("")
}

#[cfg(test)]
mod tests {
  use crate::latin_alphabet::latin_to_ascii;

  #[test]
  fn test_latin_to_ascii() {
    assert_eq!(latin_to_ascii("pokémon"), "pokemon".to_string());
    assert_eq!(latin_to_ascii("POKÉMON"), "POKEMON".to_string());
    assert_eq!(latin_to_ascii("Æther"), "AEther".to_string());
    assert_eq!(latin_to_ascii("æther"), "aether".to_string());
    // Almost exhaustive
    assert_eq!(latin_to_ascii("ÀÁÂÃÄÅ"), "AAAAAA".to_string());
    assert_eq!(latin_to_ascii("Æ"), "AE".to_string());
    assert_eq!(latin_to_ascii("Ç"), "C".to_string());
    assert_eq!(latin_to_ascii("ÈÉÊË"), "EEEE".to_string());
    assert_eq!(latin_to_ascii("ÌÍÎÏ"), "IIII".to_string());
    assert_eq!(latin_to_ascii("ÒÓÔÕÖØ"), "OOOOOO".to_string());
    assert_eq!(latin_to_ascii("ÙÚÛÜ"), "UUUU".to_string());
    assert_eq!(latin_to_ascii("Ý"), "Y".to_string());
    assert_eq!(latin_to_ascii("àáâãäå"), "aaaaaa".to_string());
    assert_eq!(latin_to_ascii("æ"), "ae".to_string());
    assert_eq!(latin_to_ascii("ç"), "c".to_string());
    assert_eq!(latin_to_ascii("èéêë"), "eeee".to_string());
    assert_eq!(latin_to_ascii("ìíîï"), "iiii".to_string());
    assert_eq!(latin_to_ascii("òóôõöø"), "oooooo".to_string());
    assert_eq!(latin_to_ascii("ùúûü"), "uuuu".to_string());
  }
}
