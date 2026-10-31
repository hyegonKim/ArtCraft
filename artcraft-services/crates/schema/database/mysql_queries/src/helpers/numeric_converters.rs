// TODO: Use macros to generate everything, or better, use a library (or stdlib) that does these.

pub fn try_i64_to_u64_or_min(value: i64) -> u64 {
  if value < 0 {
    u64::MIN
  } else {
    value as u64
  }
}

#[cfg(test)]
mod tests {
  use crate::helpers::numeric_converters::try_i64_to_u64_or_min;

  #[test]
  fn test_try_i64_to_u64_or_min() {
    // Positive
    assert_eq!(try_i64_to_u64_or_min(1i64), 1u64);
    assert_eq!(try_i64_to_u64_or_min(1000i64), 1000u64);
    assert_eq!(try_i64_to_u64_or_min(9999999i64), 9999999u64);
    assert_eq!(try_i64_to_u64_or_min(i64::MAX), 9223372036854775807u64);
    // Zero
    assert_eq!(try_i64_to_u64_or_min(0i64), 0u64);
    // Negative
    assert_eq!(try_i64_to_u64_or_min(-1i64), 0u64);
    assert_eq!(try_i64_to_u64_or_min(-1000i64), 0u64);
    assert_eq!(try_i64_to_u64_or_min(i64::MIN), 0u64);
  }
}
