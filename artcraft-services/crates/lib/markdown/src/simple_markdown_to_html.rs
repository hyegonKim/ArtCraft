use pulldown_cmark::{html, Options, Parser};

pub fn simple_markdown_to_html(markdown_input: &str) -> String {
  // NB: CommonMark allows for HTML, including <script>! WTF!
  // Let's prevent that nonsense.
  // https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html
  let markdown_input = markdown_input
      .replace("&", "&amp;")
      .replace("<", "&lt;")
      .replace(">", "&gt;")
      .replace("\"", "&quot;")
      .replace("'", "&#x27;"); // NB: &apos; is not in the HTML spec

  let mut options = Options::empty();
  options.insert(Options::ENABLE_STRIKETHROUGH);

  let parser = Parser::new_ext(&markdown_input, options);

  // NB: The buffer size can expand, but the math is a sizing heuristic I found in one of the
  // examples that may prevent buffer resizing.
  let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
  html::push_html(&mut html_output, parser);

  html_output
}

#[cfg(test)]
mod tests {
  use crate::simple_markdown_to_html::simple_markdown_to_html;

  #[test]
  fn handles_markdown() {
    assert_eq!(&simple_markdown_to_html("*italics text*"),
      "<p><em>italics text</em></p>\n");

    assert_eq!(&simple_markdown_to_html("**bold text**"),
      "<p><strong>bold text</strong></p>\n");
  }

  #[test]
  fn do_not_allow_html_entities() {
    assert_eq!(&simple_markdown_to_html("this > that"),
      "<p>this &gt; that</p>\n");

    assert_eq!(&simple_markdown_to_html("&"),
      "<p>&amp;</p>\n");

    // NB: We doubly entity decode because pulldown_cmark doesn't do enough!
    assert_eq!(&simple_markdown_to_html("&gt;"),
      "<p>&amp;gt;</p>\n");
  }

  #[test]
  fn do_not_allow_html_tags() {
    assert_eq!(&simple_markdown_to_html("<script>alert();</script>"),
      "<p>&lt;script&gt;alert();&lt;/script&gt;</p>\n");
  }
}
