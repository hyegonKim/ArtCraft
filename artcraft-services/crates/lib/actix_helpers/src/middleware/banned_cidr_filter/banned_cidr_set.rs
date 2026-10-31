use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};

use cidr_utils::cidr::IpCidr;
use cidr_utils::num_bigint::BigUint;
use num_traits::cast::ToPrimitive;

use errors::{anyhow, AnyhowResult};

#[derive(Clone)]
pub struct BannedCidrSet {
  cidr_set: Arc<RwLock<HashSet<IpCidr>>>
}

impl BannedCidrSet {
  pub fn new() -> Self {
    Self {
      cidr_set: Arc::new(RwLock::new(HashSet::new()))
    }
  }

  pub fn ip_is_banned(&self, ip_address: IpAddr) -> AnyhowResult<bool> {
    match self.cidr_set.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(cidr_set) => {
        for cidr in cidr_set.iter() {
          if cidr.contains(ip_address) {
            return Ok(true)
          }
        }
        Ok(false)
      },
    }
  }

  pub fn add_cidr(&self, ip_cidr: IpCidr) -> AnyhowResult<bool> {
    match self.cidr_set.write() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(mut cidr_set) => {
        Ok(cidr_set.insert(ip_cidr))
      },
    }
  }

  pub fn total_cidr_count(&self) -> AnyhowResult<usize> {
    match self.cidr_set.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(set) => Ok(set.len()),
    }
  }

  /// This can easily become saturated for IPv6.
  pub fn total_ip_address_count(&self) -> AnyhowResult<u128> {
    match self.cidr_set.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(cidr_set) => {
        let sum : BigUint = cidr_set.iter()
            .map(|cidr| cidr.size())
            .sum();
        let mask = BigUint::from(u128::MAX);
        let truncated = (sum & mask)
            .to_u128()
            .ok_or_else(|| anyhow!("could not truncate number"))?;
        Ok(truncated)
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use std::net::IpAddr;
  use std::str::FromStr;

  use cidr_utils::cidr::IpCidr;

  use errors::AnyhowResult;

  use crate::middleware::banned_cidr_filter::banned_cidr_set::BannedCidrSet;

  fn to_ip(ip_address: &str) -> IpAddr {
    IpAddr::from_str(ip_address).expect("ip should have parsed")
  }

  fn to_cidr(ip_address: &str) -> IpCidr {
    IpCidr::from_str(ip_address).expect("cidr should have parsed")
  }

  #[test]
  fn single_small_cidr_banned() -> AnyhowResult<()> {
    let ban_set = BannedCidrSet::new();

    ban_set.add_cidr(to_cidr("127.0.0.0/24")).expect("cdr add failed");

    // Banned
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.0"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.1"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.100"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.255"))?);

    // Not banned
    assert_eq!(false, ban_set.ip_is_banned(to_ip("127.0.1.1"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("4.4.4.4"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("1.2.3.4"))?);

    // Stats
    assert_eq!(1, ban_set.total_cidr_count()?);
    assert_eq!(256, ban_set.total_ip_address_count()?);

    Ok(())
  }

  #[test]
  fn single_big_cidr_banned() -> AnyhowResult<()> {
    let ban_set = BannedCidrSet::new();

    ban_set.add_cidr(to_cidr("127.0.0.0/8")).expect("cdr add failed");

    // Banned /24
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.0"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.1"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.100"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.255"))?);

    // Banned /8
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.1.0.0"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.100.0.1"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.127.0.100"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.255.0.255"))?);

    // Not banned
    assert_eq!(false, ban_set.ip_is_banned(to_ip("126.0.1.1"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("128.0.1.1"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("4.4.4.4"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("1.2.3.4"))?);

    // Stats
    assert_eq!(1, ban_set.total_cidr_count()?);
    assert_eq!(16777216, ban_set.total_ip_address_count()?); // That's a lot...

    Ok(())
  }

  #[test]
  fn multiple_cidrs_banned() -> AnyhowResult<()> {
    let ban_set = BannedCidrSet::new();

    ban_set.add_cidr(to_cidr("127.0.0.0/24")).expect("cdr add failed");
    ban_set.add_cidr(to_cidr("192.168.1.0/24")).expect("cdr add failed");

    // Banned (CIDR 1)
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.0"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.1"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.100"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("127.0.0.255"))?);

    // Banned (CIDR 2)
    assert_eq!(true, ban_set.ip_is_banned(to_ip("192.168.1.0"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("192.168.1.1"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("192.168.1.100"))?);
    assert_eq!(true, ban_set.ip_is_banned(to_ip("192.168.1.255"))?);

    // Not banned
    assert_eq!(false, ban_set.ip_is_banned(to_ip("127.0.1.1"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("192.168.0.1"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("192.168.2.1"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("4.4.4.4"))?);
    assert_eq!(false, ban_set.ip_is_banned(to_ip("1.2.3.4"))?);

    // Stats
    assert_eq!(2, ban_set.total_cidr_count()?);
    assert_eq!(512, ban_set.total_ip_address_count()?);

    Ok(())
  }
}
