use std::collections::HashSet;

#[derive(Clone)]
pub struct IpSet {
  pub ip_set: HashSet<String>,
}

impl IpSet {

  pub fn new() -> Self {
    Self {
      ip_set: HashSet::new(),
    }
  }

  pub fn from_set(ip_set: HashSet<String>) -> Self {
    Self {
      ip_set,
    }
  }

  pub fn replace_set(&mut self, ip_set: HashSet<String>) {
    self.ip_set = ip_set;
  }

  pub fn add_ip_address(&mut self, ip_address: String) -> bool {
    self.ip_set.insert(ip_address)
  }

  pub fn remove_ip_address(&mut self, ip_address: &str) -> bool {
    self.ip_set.remove(ip_address)
  }

  pub fn contains_ip_address<S: AsRef<str>>(&self, ip_address: S) -> bool {
    self.ip_set.contains(ip_address.as_ref())
  }

  pub fn len(&self) -> usize {
    self.ip_set.len()
  }

  pub fn is_empty(&self) -> bool {
    self.ip_set.is_empty()
  }
}
