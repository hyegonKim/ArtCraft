// TODO: Use macros to generate everything, or better, use a library (or stdlib) that does these.

/// MySQL stores non-nullable booleans as i8.
pub fn i8_to_bool(value: i8) -> bool {
  if value == 0 {
    false
  } else {
    true
  }
}

/// Bool conversion, but turn nulls to a default value.
pub fn nullable_i8_to_bool(value: Option<i8>, default_value: bool) -> bool {
  value.map(|v| i8_to_bool(v))
      .unwrap_or(default_value)
}

pub fn nullable_i8_to_bool_default_false(value: Option<i8>) -> bool {
  match value {
    None => false,
    Some(v) => match v {
      0 => false,
      _ => true,
    }
  }
}

/// Bool conversion, but retain nulls.
pub fn nullable_i8_to_optional_bool(value: Option<i8>) -> Option<bool> {
  value.map(|v| i8_to_bool(v))
}


/// MySQL "is null" returns i64.
pub fn i64_to_bool(value: i64) -> bool {
  if value == 0 {
    false
  } else {
    true
  }
}


#[cfg(test)]
mod tests {
  use crate::helpers::boolean_converters::{i64_to_bool, i8_to_bool, nullable_i8_to_bool, nullable_i8_to_bool_default_false, nullable_i8_to_optional_bool};

  #[test]
  fn test_i8_to_bool() {
    assert_eq!(i8_to_bool(0), false);
    assert_eq!(i8_to_bool(1), true);
    assert_eq!(i8_to_bool(-1), true);
    assert_eq!(i8_to_bool(120), true);
  }

  #[test]
  fn test_nullable_i8_to_bool() {
    assert_eq!(nullable_i8_to_bool(None, false), false);
    assert_eq!(nullable_i8_to_bool(None, true), true);
    assert_eq!(nullable_i8_to_bool(Some(0), false), false);
    assert_eq!(nullable_i8_to_bool(Some(0), true), false);

    assert_eq!(nullable_i8_to_bool(Some(1), false), true);
    assert_eq!(nullable_i8_to_bool(Some(1), true), true);

    assert_eq!(nullable_i8_to_bool(Some(100), false), true);
    assert_eq!(nullable_i8_to_bool(Some(100), true), true);
    assert_eq!(nullable_i8_to_bool(Some(-100), false), true);
    assert_eq!(nullable_i8_to_bool(Some(-100), true), true);
  }

  #[test]
  fn test_nullable_i8_to_bool_default_false() {
    assert_eq!(nullable_i8_to_bool_default_false(None), false);
    assert_eq!(nullable_i8_to_bool_default_false(Some(0)), false);

    assert_eq!(nullable_i8_to_bool_default_false(Some(1)), true);
    assert_eq!(nullable_i8_to_bool_default_false(Some(100)), true);
    assert_eq!(nullable_i8_to_bool_default_false(Some(-100)), true);
  }

  #[test]
  fn test_nullable_i8_to_optional_bool() {
    assert_eq!(nullable_i8_to_optional_bool(None), None);
    assert_eq!(nullable_i8_to_optional_bool(Some(0)), Some(false));
    assert_eq!(nullable_i8_to_optional_bool(Some(1)), Some(true));
    assert_eq!(nullable_i8_to_optional_bool(Some(100)), Some(true));
    assert_eq!(nullable_i8_to_optional_bool(Some(-100)), Some(true));
  }

  #[test]
  fn test_i64_to_bool() {
    assert_eq!(i64_to_bool(0), false);
    assert_eq!(i64_to_bool(1), true);
    assert_eq!(i64_to_bool(-1), true);
    assert_eq!(i64_to_bool(120), true);
  }
}
