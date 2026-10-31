use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::types::ids::{UserId, WorkspaceId};
use chrono::{DateTime, Duration, Utc};
use jwt_light::common_claims::CommonClaims;
use jwt_light::error::JwtError;
use jwt_light::parse_jwt_claims_trait::ParseJwtClaims;
use serde_json::{Map, Value};

/// A Clerk session JWT with its (unverified) claims decoded.
///
/// This is the bearer token the API gateway wants. Clerk issues them with a
/// ~60 second lifetime; use [`Self::is_fresh`] before sending one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClerkSessionToken {
  jwt: String,
  claims: ClerkSessionClaims,
}

/// The claims we care about. NB: decoded without signature verification —
/// fine for reading our own session's metadata, never for trusting input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClerkSessionClaims {
  pub issued_at: DateTime<Utc>,
  pub expires_at: DateTime<Utc>,

  /// Clerk session id (`sid`), e.g. `sess_...`. Needed to mint new tokens.
  pub session_id: String,

  /// Clerk user id (`sub`), e.g. `user_...`.
  pub user_id: UserId,

  /// Issuer (`iss`), e.g. `https://clerk.higgsfield.ai`.
  pub maybe_issuer: Option<String>,

  pub maybe_email: Option<String>,

  pub maybe_full_name: Option<String>,

  /// Higgsfield's custom `workspace_id` claim.
  pub maybe_workspace_id: Option<WorkspaceId>,
}

impl ClerkSessionToken {
  /// Decode a raw JWT. Fails if it isn't a three-part JWT with `exp`, `iat`,
  /// `sid`, and `sub` claims.
  pub fn parse(jwt: impl Into<String>) -> Result<Self, HiggsfieldClientError> {
    let jwt = jwt.into().trim().to_string();
    let claims = ClerkSessionClaims::parse_claims(&jwt)
        .map_err(|err| HiggsfieldClientError::InvalidSessionToken(format!("{:?}", err)))?;
    Ok(Self { jwt, claims })
  }

  pub fn jwt(&self) -> &str {
    &self.jwt
  }

  pub fn into_jwt(self) -> String {
    self.jwt
  }

  pub fn claims(&self) -> &ClerkSessionClaims {
    &self.claims
  }

  pub fn session_id(&self) -> &str {
    &self.claims.session_id
  }

  pub fn expires_at(&self) -> DateTime<Utc> {
    self.claims.expires_at
  }

  /// Whether the token is still good for at least `margin` more. Callers
  /// mint a new one when this is false.
  pub fn is_fresh(&self, now: DateTime<Utc>, margin: Duration) -> bool {
    now + margin < self.claims.expires_at
  }

  /// Time left before expiry (zero once expired).
  pub fn remaining(&self, now: DateTime<Utc>) -> Duration {
    (self.claims.expires_at - now).max(Duration::zero())
  }
}

impl ParseJwtClaims for ClerkSessionClaims {
  fn extract_claims(common_claims: CommonClaims, extra_claims: Map<String, Value>) -> Result<Self, JwtError> {
    let string_claim = |name: &str| -> Option<String> {
      extra_claims.get(name).and_then(|value| value.as_str()).map(|s| s.to_string())
    };

    let session_id = string_claim("sid")
        .ok_or_else(|| JwtError::CustomClaimsFieldError("no sid claim".to_string()))?;
    let user_id = string_claim("sub")
        .ok_or_else(|| JwtError::CustomClaimsFieldError("no sub claim".to_string()))?;

    Ok(Self {
      issued_at: common_claims.created,
      expires_at: common_claims.expiration,
      session_id,
      user_id: UserId::new(user_id),
      maybe_issuer: string_claim("iss"),
      maybe_email: string_claim("email"),
      maybe_full_name: string_claim("full_name"),
      maybe_workspace_id: string_claim("workspace_id").map(WorkspaceId::new),
    })
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use super::*;
  use base64::Engine;
  use base64::prelude::BASE64_URL_SAFE_NO_PAD;

  /// Build an unsigned JWT with the given claims, shaped like Clerk's.
  pub(crate) fn fake_clerk_jwt(claims: &Value) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","kid":"ins_TEST","typ":"JWT"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode(claims.to_string());
    format!("{header}.{payload}.signature")
  }

  /// A token that expires at `exp` (epoch seconds), issued 60s earlier.
  pub(crate) fn fake_clerk_token(exp: i64) -> ClerkSessionToken {
    let jwt = fake_clerk_jwt(&serde_json::json!({
      "azp": "https://higgsfield.ai",
      "email": "user@example.com",
      "exp": exp,
      "full_name": "Test User",
      "iat": exp - 60,
      "iss": "https://clerk.higgsfield.ai",
      "jti": "0000000000000000000000",
      "nbf": exp - 70,
      "sid": "sess_TESTSESSION000000000000000",
      "sts": "active",
      "sub": "user_TESTUSER0000000000000000000",
      "workspace_id": "00000000-0000-0000-0000-00000000aaaa"
    }));
    ClerkSessionToken::parse(jwt).unwrap()
  }

  #[test]
  fn parses_clerk_shaped_claims() {
    let token = fake_clerk_token(1_788_147_273);
    let claims = token.claims();
    assert_eq!(claims.session_id, "sess_TESTSESSION000000000000000");
    assert_eq!(claims.user_id.as_str(), "user_TESTUSER0000000000000000000");
    assert_eq!(claims.maybe_email.as_deref(), Some("user@example.com"));
    assert_eq!(claims.maybe_full_name.as_deref(), Some("Test User"));
    assert_eq!(claims.maybe_issuer.as_deref(), Some("https://clerk.higgsfield.ai"));
    assert_eq!(claims.maybe_workspace_id.as_ref().unwrap().as_str(), "00000000-0000-0000-0000-00000000aaaa");
    assert_eq!(claims.expires_at, DateTime::from_timestamp(1_788_147_273, 0).unwrap());
    assert_eq!(claims.issued_at, DateTime::from_timestamp(1_788_147_213, 0).unwrap());
  }

  #[test]
  fn freshness_uses_margin() {
    let token = fake_clerk_token(1_000_000);
    let expires = DateTime::from_timestamp(1_000_000, 0).unwrap();
    assert!(token.is_fresh(expires - Duration::seconds(30), Duration::seconds(10)));
    assert!(!token.is_fresh(expires - Duration::seconds(5), Duration::seconds(10)));
    assert!(!token.is_fresh(expires + Duration::seconds(1), Duration::zero()));
    assert_eq!(token.remaining(expires - Duration::seconds(30)), Duration::seconds(30));
    assert_eq!(token.remaining(expires + Duration::seconds(30)), Duration::zero());
  }

  #[test]
  fn rejects_non_jwt_and_missing_claims() {
    assert!(matches!(ClerkSessionToken::parse("not-a-jwt"), Err(HiggsfieldClientError::InvalidSessionToken(_))));
    let no_sid = fake_clerk_jwt(&serde_json::json!({"exp": 10, "iat": 5, "sub": "user_x"}));
    assert!(matches!(ClerkSessionToken::parse(no_sid), Err(HiggsfieldClientError::InvalidSessionToken(_))));
  }
}
