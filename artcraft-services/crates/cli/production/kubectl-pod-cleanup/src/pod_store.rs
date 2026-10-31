use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::{Arc, RwLock};

use rand::prelude::IteratorRandom;
use rand::thread_rng;

use errors::{anyhow, AnyhowResult};

#[derive(Clone)]
pub struct PodStore {
  pod_names: Arc<RwLock<HashSet<String>>>,
}

impl PodStore {
  pub fn new() -> Self {
    Self {
      pod_names: Arc::new(RwLock::new(HashSet::new()))
    }
  }

  pub fn from_names(pod_names: Vec<String>) -> Self {
    Self {
      pod_names: Arc::new(RwLock::new(HashSet::from_iter(pod_names)))
    }
  }

  pub fn replace_pods(&self, pod_names: Vec<String>) -> AnyhowResult<()> {
    match self.pod_names.write() {
      Err(err) => {
        Err(anyhow!("lock error: {:?}", err))
      }
      Ok(mut write) => {
        write.clear();
        write.extend(pod_names);
        Ok(())
      }
    }
  }

  pub fn grab_batch(&self, batch_size: usize) -> AnyhowResult<HashSet<String>> {
    match self.pod_names.read() {
      Err(err) => {
        Err(anyhow!("lock error: {:?}", err))
      }
      Ok(read) => {
        let mut batch : HashSet<String> = HashSet::with_capacity(batch_size);
        let choices = read.iter()
            .choose_multiple(&mut thread_rng(), batch_size)
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        batch.extend(choices);

        Ok(batch)
      }
    }
  }

  pub fn expunge_pods(&self, pod_names: &Vec<String>) -> AnyhowResult<()> {
    match self.pod_names.write() {
      Err(err) => {
        Err(anyhow!("lock error: {:?}", err))
      }
      Ok(mut write) => {
        for pod_name in pod_names {
          write.remove(pod_name);
        }
        Ok(())
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::pod_store::PodStore;

  #[test]
  fn test_grab_batch() {
    let pod_names = vec!["foo", "bar", "bin", "baz"].into_iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>();

    let pod_names = PodStore::from_names(pod_names);

    assert_eq!(pod_names.grab_batch(1).unwrap().len(), 1);
    assert_eq!(pod_names.grab_batch(2).unwrap().len(), 2);
    assert_eq!(pod_names.grab_batch(3).unwrap().len(), 3);
    assert_eq!(pod_names.grab_batch(4).unwrap().len(), 4);
    assert_eq!(pod_names.grab_batch(5).unwrap().len(), 4); // Only four items total
    assert_eq!(pod_names.grab_batch(100).unwrap().len(), 4); // Only four items total
  }

  #[test]
  fn test_expunge_pods() {
    let pod_names = vec!["foo", "bar", "bin", "baz"].into_iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>();

    let pod_names = PodStore::from_names(pod_names);

    assert_eq!(pod_names.grab_batch(10).unwrap().len(), 4);

    // NB: Not in the list
    pod_names.expunge_pods(&vec!["asdf".to_string()]).unwrap();
    assert_eq!(pod_names.grab_batch(10).unwrap().len(), 4);

    pod_names.expunge_pods(&vec!["foo".to_string()]).unwrap();
    assert_eq!(pod_names.grab_batch(10).unwrap().len(), 3);

    pod_names.expunge_pods(&vec!["bar".to_string()]).unwrap();
    pod_names.expunge_pods(&vec!["bin".to_string()]).unwrap();
    assert_eq!(pod_names.grab_batch(10).unwrap().len(), 1);

    pod_names.expunge_pods(&vec!["baz".to_string()]).unwrap();
    pod_names.expunge_pods(&vec!["test".to_string()]).unwrap();
    assert_eq!(pod_names.grab_batch(10).unwrap().len(), 0);
  }
}