//! Compile-time domain name validation.
//!
//! `assert_valid_domain` is a `const fn` that panics on an invalid name. The
//! `domain!` macro calls it while evaluating a `const`/`static`, where a panic
//! is a compile error, so a bad domain literal never builds.

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

/// Panics (a compile error in const context) unless `domain` is a canonical,
/// fully-qualified DNS name: lowercase `a-z`, `0-9` and `-` labels separated
/// by single dots, at least two labels, no scheme, port, path or trailing dot,
/// and a non-numeric top-level label (so IP addresses are rejected).
pub const fn assert_valid_domain(domain: &str) {
  match check_domain(domain) {
    Ok(()) => {}
    Err(DomainError::Empty) => panic!("invalid domain: empty"),
    Err(DomainError::TooLong) => panic!("invalid domain: longer than 253 bytes"),
    Err(DomainError::EmptyLabel) => panic!("invalid domain: empty label (leading, trailing or doubled dot)"),
    Err(DomainError::LabelTooLong) => panic!("invalid domain: label longer than 63 bytes"),
    Err(DomainError::InvalidCharacter) => panic!("invalid domain: only lowercase a-z, 0-9, '-' and '.' are allowed"),
    Err(DomainError::HyphenAtLabelEdge) => panic!("invalid domain: label starts or ends with '-'"),
    Err(DomainError::SingleLabel) => panic!("invalid domain: needs at least two labels (eg. example.com)"),
    Err(DomainError::NumericTopLevelLabel) => panic!("invalid domain: top-level label is numeric (IP addresses are not domains)"),
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum DomainError {
  Empty,
  TooLong,
  EmptyLabel,
  LabelTooLong,
  InvalidCharacter,
  HyphenAtLabelEdge,
  SingleLabel,
  NumericTopLevelLabel,
}

pub(crate) const fn check_domain(domain: &str) -> Result<(), DomainError> {
  let bytes = domain.as_bytes();
  if bytes.is_empty() {
    return Err(DomainError::Empty);
  }
  if bytes.len() > MAX_DOMAIN_LEN {
    return Err(DomainError::TooLong);
  }

  let mut labels = 0;
  let mut label_start = 0;
  let mut last_label_start = 0;
  let mut i = 0;
  // Walk one past the end so the final label is checked like the others.
  while i <= bytes.len() {
    if i == bytes.len() || bytes[i] == b'.' {
      match check_label(bytes, label_start, i) {
        Ok(()) => {}
        Err(err) => return Err(err),
      }
      labels += 1;
      last_label_start = label_start;
      label_start = i + 1;
    }
    i += 1;
  }

  if labels < 2 {
    return Err(DomainError::SingleLabel);
  }
  if is_all_digits(bytes, last_label_start, bytes.len()) {
    return Err(DomainError::NumericTopLevelLabel);
  }
  Ok(())
}

/// Checks `bytes[start..end]` as a single label.
const fn check_label(bytes: &[u8], start: usize, end: usize) -> Result<(), DomainError> {
  let len = end - start;
  if len == 0 {
    return Err(DomainError::EmptyLabel);
  }
  if len > MAX_LABEL_LEN {
    return Err(DomainError::LabelTooLong);
  }
  let mut i = start;
  while i < end {
    match bytes[i] {
      b'a'..=b'z' | b'0'..=b'9' | b'-' => {}
      _ => return Err(DomainError::InvalidCharacter),
    }
    i += 1;
  }
  if bytes[start] == b'-' || bytes[end - 1] == b'-' {
    return Err(DomainError::HyphenAtLabelEdge);
  }
  Ok(())
}

const fn is_all_digits(bytes: &[u8], start: usize, end: usize) -> bool {
  let mut i = start;
  while i < end {
    if !bytes[i].is_ascii_digit() {
      return false;
    }
    i += 1;
  }
  true
}

#[cfg(test)]
mod tests {
  use super::DomainError::*;
  use super::check_domain;

  mod valid {
    use super::*;

    #[test]
    fn apex_domain() {
      assert_eq!(check_domain("getartcraft.com"), Ok(()));
    }

    #[test]
    fn subdomains() {
      assert_eq!(check_domain("desktop.getartcraft.com"), Ok(()));
      assert_eq!(check_domain("studio-staging.studio.storyteller.ai"), Ok(()));
    }

    #[test]
    fn digits_and_hyphens_inside_labels() {
      assert_eq!(check_domain("cdn-2.fakeyou.com"), Ok(()));
      assert_eq!(check_domain("3d.getartcraft.com"), Ok(()));
      assert_eq!(check_domain("pub-c8a4a5bdbdb048f286b77bdf9f786ff2.r2.dev"), Ok(()));
    }

    #[test]
    fn max_lengths() {
      let label_63 = "a".repeat(63);
      assert_eq!(check_domain(&format!("{label_63}.com")), Ok(()));

      // 63 + 1 + 63 + 1 + 63 + 1 + 61 = 253
      let domain_253 = format!("{label_63}.{label_63}.{label_63}.{}", "a".repeat(61));
      assert_eq!(domain_253.len(), 253);
      assert_eq!(check_domain(&domain_253), Ok(()));
    }
  }

  mod invalid {
    use super::*;

    #[test]
    fn empty() {
      assert_eq!(check_domain(""), Err(Empty));
    }

    #[test]
    fn too_long() {
      let label_63 = "a".repeat(63);
      let domain_254 = format!("{label_63}.{label_63}.{label_63}.{}", "a".repeat(62));
      assert_eq!(check_domain(&domain_254), Err(TooLong));
    }

    #[test]
    fn empty_labels() {
      assert_eq!(check_domain(".getartcraft.com"), Err(EmptyLabel));
      assert_eq!(check_domain("getartcraft.com."), Err(EmptyLabel));
      assert_eq!(check_domain("desktop..getartcraft.com"), Err(EmptyLabel));
      assert_eq!(check_domain("."), Err(EmptyLabel));
    }

    #[test]
    fn label_too_long() {
      assert_eq!(check_domain(&format!("{}.com", "a".repeat(64))), Err(LabelTooLong));
    }

    #[test]
    fn uppercase() {
      assert_eq!(check_domain("GetArtCraft.com"), Err(InvalidCharacter));
    }

    #[test]
    fn scheme_port_path_and_whitespace() {
      assert_eq!(check_domain("https://getartcraft.com"), Err(InvalidCharacter));
      assert_eq!(check_domain("getartcraft.com:443"), Err(InvalidCharacter));
      assert_eq!(check_domain("getartcraft.com/"), Err(InvalidCharacter));
      assert_eq!(check_domain(" getartcraft.com"), Err(InvalidCharacter));
    }

    #[test]
    fn non_ascii() {
      assert_eq!(check_domain("artcräft.com"), Err(InvalidCharacter));
    }

    #[test]
    fn underscore() {
      assert_eq!(check_domain("dev_proxy.fakeyou.com"), Err(InvalidCharacter));
    }

    #[test]
    fn hyphen_at_label_edge() {
      assert_eq!(check_domain("-desktop.getartcraft.com"), Err(HyphenAtLabelEdge));
      assert_eq!(check_domain("desktop-.getartcraft.com"), Err(HyphenAtLabelEdge));
      assert_eq!(check_domain("getartcraft.com-"), Err(HyphenAtLabelEdge));
    }

    #[test]
    fn single_label() {
      assert_eq!(check_domain("localhost"), Err(SingleLabel));
    }

    #[test]
    fn ip_addresses() {
      assert_eq!(check_domain("127.0.0.1"), Err(NumericTopLevelLabel));
      assert_eq!(check_domain("example.123"), Err(NumericTopLevelLabel));
    }
  }
}
