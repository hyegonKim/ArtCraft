use base64::Engine;
use chrono::{DateTime, Utc};
use jwt_light::common_claims::CommonClaims;
use jwt_light::error::JwtError;
use jwt_light::parse_jwt_claims_trait::ParseJwtClaims;
use serde_json::{Map, Value};


#[derive(Clone, Debug)]
pub struct JwtClaims {
  /// From the "iat" field
  pub created: DateTime<Utc>,
  /// From the "exp" field
  pub expiration: DateTime<Utc>,
  /// From the "id" field
  pub id: String,
  /// From the "email" field
  pub email: String,
  /// From the "sso" field
  pub sso: bool,
}

impl ParseJwtClaims for JwtClaims {
  fn extract_claims(common_claims: CommonClaims, extra_claims: Map<String, Value>) -> Result<Self, JwtError> {
    let id = extra_claims.get("id")
        .ok_or_else(|| JwtError::CustomClaimsFieldError("no id claim".to_string()))?
        .as_i64()
        .ok_or_else(|| JwtError::CustomClaimsFieldError("id is not a string".to_string()))?
        .to_string();

    let email = extra_claims.get("email")
        .ok_or_else(|| JwtError::CustomClaimsFieldError("no email claim".to_string()))?
        .as_str()
        .ok_or_else(|| JwtError::CustomClaimsFieldError("email is not a string".to_string()))?
        .to_string();

    let sso = extra_claims.get("sso")
        .map(|val| val.as_bool())
        .flatten()
        .unwrap_or(false);

    Ok(Self {
      created: common_claims.created,
      expiration: common_claims.expiration,
      id,
      email,
      sso,
    })
  }
}

#[cfg(test)]
mod tests {
  use crate::creds::jwt_claims::JwtClaims;
  use jwt_light::parse_jwt_claims_trait::ParseJwtClaims;

  #[test]
  fn test_token() {
    /*{
      "id": 23129405,
      "email": "echelon@gmail.com",
      "exp": 1750201256.49,
      "iat": 1747609256.49,
      "sso": false
    }*/
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpZCI6MjMxMjk0MDUsImVtYWlsIjoiZWNoZWxvbkBnbWFpbC5jb20iLCJleHAiOjE3NTAyMDEyNTYuNDksImlhdCI6MTc0NzYwOTI1Ni40OSwic3NvIjpmYWxzZX0.UxbJozIiHSApqI8_Vl7o2d7q7CzqpXIzsZoazCtY75s";
    let claims = JwtClaims::parse_claims(jwt).unwrap();

    assert_eq!(claims.id, "23129405");
    assert_eq!(claims.email, "echelon@gmail.com");
    assert_eq!(claims.sso, false);
    assert_eq!(claims.created.timestamp(), 1747609256);
    assert_eq!(claims.expiration.timestamp(), 1750201256);
  }
}
