use rand::prelude::SliceRandom;

/// Return a random item from a array.
pub fn random_from_array<T>(array: &[T]) -> Option<&T> {
  array.choose(&mut rand::thread_rng())
}

#[cfg(test)]
mod tests {
  use crate::random_from_array::random_from_array;

  #[test]
  fn pulls_item_from_array() {
    let items: [&str; 3] = ["one", "two", "three"];
    let selected = random_from_array(&items).unwrap();
    assert!(items.contains(selected));

    let items : [String; 3] = ["one".to_string(), "two".to_string(), "three".to_string()];

    let selected = random_from_array(&items).unwrap();
    assert!(items.contains(selected));
  }

  #[test]
  fn behavior_on_empty() {
    let items : [String; 0] = [];
    let selected = random_from_array(&items);
    assert!(selected.is_none());
  }
}
