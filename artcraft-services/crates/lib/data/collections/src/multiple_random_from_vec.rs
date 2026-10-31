use rand::seq::SliceRandom;

/// Return multiple random item from a vec. May not return the number of items requested.
/// Underlying doc: "Produces an iterator that chooses amount elements from the slice at random
/// without repeating any, and returns them in random order."
///
/// NB: This will not be as efficient as the underlying method due to Vec<> allocation.
///  Use only as convenience.
pub fn multiple_random_from_vec<T>(vec: &Vec<T>, amount: usize) -> Vec<&T> {
  vec.choose_multiple(&mut rand::thread_rng(), amount).collect()
}

#[cfg(test)]
mod tests {
  use crate::multiple_random_from_vec::multiple_random_from_vec;

  #[test]
  fn pulls_item_from_vec() {
    let items = vec!["one", "two", "three", "four"];

    let selected = multiple_random_from_vec(&items, 1);
    assert_eq!(selected.len(), 1);

    let selected = multiple_random_from_vec(&items, 2);
    assert_eq!(selected.len(), 2);

    let selected = multiple_random_from_vec(&items, 3);
    assert_eq!(selected.len(), 3);

    let selected = multiple_random_from_vec(&items, 4);
    assert_eq!(selected.len(), 4);

    let selected = multiple_random_from_vec(&items, 5);
    assert_eq!(selected.len(), 4); // NB: Max is four!
  }

  #[test]
  fn behavior_on_empty() {
    let items : Vec<String> = vec![];
    let selected = multiple_random_from_vec(&items, 5);
    assert!(selected.is_empty());
  }

  fn behavior_on_zero_requested() {
    let items = vec!["one", "two", "three"];
    let selected = multiple_random_from_vec(&items, 0);
    assert!(selected.is_empty());
  }
}
