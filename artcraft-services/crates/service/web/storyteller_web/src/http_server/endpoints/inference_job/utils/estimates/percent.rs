use std::cmp::min;

pub fn percent(numerator: u64, denominator: u64) -> u8 {
  if denominator == 0 {
    return 0;
  }
  let percent = (numerator as f64 / denominator as f64) * 100.0;
  let percent = percent as u8;

  min(percent, 100)
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_percent() {
    assert_eq!(0, super::percent(0, 0));
    assert_eq!(0, super::percent(0, 100));
    assert_eq!(50, super::percent(50, 100));
    assert_eq!(100, super::percent(100, 100));
    assert_eq!(100, super::percent(500, 100));
    assert_eq!(25, super::percent(1, 4));
    assert_eq!(50, super::percent(2, 4));
    assert_eq!(75, super::percent(3, 4));
    assert_eq!(100, super::percent(4, 4));
    assert_eq!(100, super::percent(5, 4));
  }
}
