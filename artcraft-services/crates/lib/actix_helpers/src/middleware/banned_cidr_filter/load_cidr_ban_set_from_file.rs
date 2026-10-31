use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use cidr_utils::cidr::IpCidr;

use errors::AnyhowResult;

use crate::middleware::banned_cidr_filter::banned_cidr_set::BannedCidrSet;

pub fn load_cidr_ban_set_from_file<P: AsRef<Path>>(path: P) -> AnyhowResult<BannedCidrSet> {
  let file = File::open(path)?;
  let reader = BufReader::new(file);

  let lines = reader.lines()
      .filter_map(|line| line.ok())
      .map(|line| line.trim().to_string())
      .filter(|line| !(line.starts_with("#") || line.is_empty()))
      .collect::<HashSet<String>>();

  let cidr_bans = BannedCidrSet::new();

  for line in lines.iter() {
    let _ = cidr_bans.add_cidr(IpCidr::from_str(line)?)?;
  }

  Ok(cidr_bans)
}

#[cfg(test)]
mod tests {
  use std::net::IpAddr;
  use std::path::PathBuf;
  use std::str::FromStr;

  use errors::AnyhowResult;

  use crate::middleware::banned_cidr_filter::load_cidr_ban_set_from_file::load_cidr_ban_set_from_file;

  fn test_file(path_from_repo_root: &str) -> PathBuf {
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("../../../{}", path_from_repo_root));
    path
  }

  fn to_ip(ip_address: &str) -> IpAddr {
    IpAddr::from_str(ip_address).expect("ip should have parsed")
  }

  #[test]
  fn test_load_ip_set_from_file() -> AnyhowResult<()> {
    let filename = test_file("test_data/text_files/cidr_ban_example/local_cidrs_example.txt");
    let cidr_bans = load_cidr_ban_set_from_file(filename).unwrap();

    // Banned CIDRs (1)
    assert_eq!(true, cidr_bans.ip_is_banned(to_ip("127.0.0.0"))?);
    assert_eq!(true, cidr_bans.ip_is_banned(to_ip("127.0.0.1"))?);
    assert_eq!(true, cidr_bans.ip_is_banned(to_ip("127.0.0.100"))?);

    // Banned CIDRs (2)
    assert_eq!(true, cidr_bans.ip_is_banned(to_ip("192.168.0.1"))?);
    assert_eq!(true, cidr_bans.ip_is_banned(to_ip("192.168.0.100"))?);

    // Permitted
    assert_eq!(false, cidr_bans.ip_is_banned(to_ip("1.2.3.4"))?);
    assert_eq!(false, cidr_bans.ip_is_banned(to_ip("4.4.4.4"))?);
    assert_eq!(false, cidr_bans.ip_is_banned(to_ip("255.255.255.255"))?);

    // Stats from file loaded
    assert_eq!(2, cidr_bans.total_cidr_count()?);
    assert_eq!(512, cidr_bans.total_ip_address_count()?);

    Ok(())
  }
}
