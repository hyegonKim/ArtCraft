use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use errors::{anyhow, AnyhowResult};

use crate::middleware::banned_ip_filter::ip_ban_list::ip_set::IpSet;

#[derive(Clone)]
pub struct IpBanList {
  ip_sets: Arc<RwLock<HashMap<String, IpSet>>>
}

impl IpBanList {
  pub fn new() -> Self {
    Self {
      ip_sets: Arc::new(RwLock::new(HashMap::new()))
    }
  }

  pub fn contains_ip_address<S: AsRef<str>>(&self, ip_address: S) -> AnyhowResult<bool> {
    match self.ip_sets.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(sets) => {
        for set in sets.values() {
          if set.contains_ip_address(ip_address.as_ref()) {
            return Ok(true)
          }
        }
        Ok(false)
      },
    }
  }

  pub fn add_set(&self, set_name: String, ip_set: IpSet) -> AnyhowResult<Option<IpSet>> {
    match self.ip_sets.write() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(mut sets) => {
        Ok(sets.insert(set_name, ip_set))
      },
    }
  }

  pub fn remove_set(&self, set_name: &str) -> AnyhowResult<Option<IpSet>> {
    match self.ip_sets.write() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(mut sets) => {
        Ok(sets.remove(set_name))
      },
    }
  }

  pub fn total_ip_address_count(&self) -> AnyhowResult<usize> {
    match self.ip_sets.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(sets) => {
        Ok(sets.values()
            .map(|set| set.len())
            .sum())
      },
    }
  }

  pub fn set_count(&self) -> AnyhowResult<usize> {
    match self.ip_sets.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(sets) => {
        Ok(sets.len())
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::middleware::banned_ip_filter::ip_ban_list::ip_ban_list::IpBanList;
  use crate::middleware::banned_ip_filter::ip_ban_list::ip_set::IpSet;

  #[test]
  fn test_contains_ip() {
    let mut local_ip_set = IpSet::new();
    local_ip_set.add_ip_address("127.0.0.1".to_string());
    local_ip_set.add_ip_address("192.168.0.1".to_string());

    let mut remote_ip_set = IpSet::new();
    remote_ip_set.add_ip_address("4.4.4.4".to_string());
    remote_ip_set.add_ip_address("8.8.8.8".to_string());

    let ip_ban_list = IpBanList::new();
    ip_ban_list.add_set("local".to_string(), local_ip_set).unwrap();
    ip_ban_list.add_set("remote".to_string(), remote_ip_set).unwrap();

    assert_eq!(ip_ban_list.contains_ip_address("192.168.0.1").unwrap(), true);
    assert_eq!(ip_ban_list.contains_ip_address("8.8.8.8").unwrap(), true);

    assert_eq!(ip_ban_list.contains_ip_address("1.2.3.4").unwrap(), false);
  }
}
