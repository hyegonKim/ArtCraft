// Stored credentials (one TOML file per credential in the desktop app's
// credentials directory). Hidden from users; the effective primary identifier.
define_id!(pub struct CredentialId => "credential");

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;

  #[test]
  fn generate_has_prefix() {
    assert!(CredentialId::generate().as_str().starts_with("credential_"));
    assert_eq!(CredentialId::PREFIX, "credential");
  }

  #[test]
  fn from_str_roundtrip() {
    let id = CredentialId::generate();
    let parsed = CredentialId::from_str(id.as_str()).unwrap();
    assert_eq!(parsed, id);
  }

  #[test]
  fn from_str_rejects_wrong_prefix() {
    assert!(CredentialId::from_str("user_01j9abcdefghjkmnpqrstvwxyz").is_err());
  }

  #[test]
  fn from_str_rejects_short_entropy() {
    assert!(CredentialId::from_str("credential_tooshort").is_err());
  }

  #[test]
  fn from_trusted_skips_validation() {
    let id = CredentialId::from_trusted("credential_legacy21charentropy00");
    assert_eq!(id.as_str(), "credential_legacy21charentropy00");
  }

  #[test]
  fn serializes_as_bare_string() {
    let id = CredentialId::generate();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, format!("\"{id}\""));
    let back: CredentialId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
  }

  #[test]
  fn ids_are_unique() {
    assert_ne!(CredentialId::generate(), CredentialId::generate());
  }
}
