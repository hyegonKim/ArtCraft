use chrono::NaiveDate;

/// Generate all dates between start and end, with both ends inclusive.
/// This consumes a lot of memory, so it probably isn't the best
/// methodology as opposed to an iterator.
pub fn generate_dates_inclusive(date_start: NaiveDate, date_end: NaiveDate) -> Vec<NaiveDate> {
  let difference = date_end.signed_duration_since(date_start);
  let mut dates = Vec::with_capacity(difference.num_days() as usize);

  let mut current_date = date_start;
  while current_date <= date_end {
    dates.push(current_date);
    match current_date.succ_opt() {
      Some(next_date) => current_date = next_date,
      None => break,
    }
  }

  dates
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;

  use crate::generate_dates_inclusive::generate_dates_inclusive;

  #[test]
  fn three_days() {
    let start = NaiveDate::from_ymd(2021, 1, 1);
    let end = NaiveDate::from_ymd(2021, 1, 3);

    let dates = generate_dates_inclusive(start, end);

    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], start);
    assert_eq!(dates[1], NaiveDate::from_ymd(2021, 1, 2));
    assert_eq!(dates[2], end);
  }

  #[test]
  fn zero_days() {
    let start = NaiveDate::from_ymd(2021, 1, 1);
    let end = NaiveDate::from_ymd(2021, 1, 1);

    let dates = generate_dates_inclusive(start, end);

    assert_eq!(dates.len(), 1);
    assert_eq!(dates[0], start);
    assert_eq!(dates[0], end);
  }

  #[test]
  fn one_year() {
    let start = NaiveDate::from_ymd(2001, 1, 1);
    let end = NaiveDate::from_ymd(2001, 12, 31);

    let dates = generate_dates_inclusive(start, end);

    assert_eq!(dates.len(), 365);
    assert_eq!(dates[0], start);
    assert_eq!(dates[100], NaiveDate::from_ymd(2001, 4, 11));
    assert_eq!(dates[200], NaiveDate::from_ymd(2001, 7, 20));
    assert_eq!(dates[300], NaiveDate::from_ymd(2001, 10, 28));
    assert_eq!(dates[364], end);
  }

  #[test]
  fn many_years() {
    let start = NaiveDate::from_ymd(1998, 1, 1);
    let end = NaiveDate::from_ymd(2020, 12, 31);

    let dates = generate_dates_inclusive(start, end);

    assert_eq!(dates.len(), 8401);
    assert_eq!(dates[0], start);
    assert_eq!(dates[2000], NaiveDate::from_ymd(2003, 6, 24));
    assert_eq!(dates[4000], NaiveDate::from_ymd(2008, 12, 14));
    assert_eq!(dates[6000], NaiveDate::from_ymd(2014, 6, 6));
    assert_eq!(dates[8400], end);
  }
}
