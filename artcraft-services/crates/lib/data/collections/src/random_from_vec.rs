use rand::seq::SliceRandom;

/// Return a random item from a vec.
pub fn random_from_vec<T>(vec: &Vec<T>) -> Option<&T> {
  vec.choose(&mut rand::thread_rng())
}

#[cfg(test)]
mod tests {
  use crate::random_from_vec::random_from_vec;

  #[test]
  fn pulls_item_from_vec() {
    let items = vec!["one", "two", "three"];
    let selected = random_from_vec(&items).unwrap();
    assert!(items.contains(selected));

    let items = vec!["one", "two", "three"].into_iter()
        .map(|s| s.to_string())
        .collect();
    let selected = random_from_vec(&items).unwrap();
    assert!(items.contains(selected));
  }

  #[test]
  fn behavior_on_empty() {
    let items : Vec<String> = vec![];
    let selected = random_from_vec(&items);
    assert!(selected.is_none());
  }
}