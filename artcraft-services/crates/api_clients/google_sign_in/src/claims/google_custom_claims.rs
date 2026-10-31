use serde_derive::{Deserialize, Serialize};

/// Extra Payload of Google Sign In JWT
/// https://developers.google.com/identity/openid-connect/openid-connect
/// https://stackoverflow.com/questions/31056412/what-all-these-fields-mean
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GoogleCustomClaims {
  /// The user's email address. Provided only if you included the email scope in your
  /// request. The value of this claim may not be unique to this account and could
  /// change over time, therefore you should not use this value as the primary identifier
  /// to link to your user record. You also can't rely on the domain of the email claim
  /// to identify users of Google Workspace or Cloud organizations; use the hd claim instead.
  pub email: Option<String>,

  /// True if the user's e-mail address has been verified; otherwise false.
  pub email_verified: Option<bool>,

  /// The user's full name, in a displayable form. (See Google documentation)
  pub name: Option<String>,

  /// The user's given name(s) or first name(s). Might be provided when a name claim is present.
  pub given_name: Option<String>,

  /// The user's surname(s) or last name(s). Might be provided when a name claim is present.
  pub family_name: Option<String>,

  /// The client_id of the authorized presenter. This claim is only needed when the party
  /// requesting the ID token is not the same as the audience of the ID token. This may be
  /// the case at Google for hybrid apps where a web application and Android app have a
  /// different OAuth 2.0 client_id but share the same Google APIs project.
  pub azp: Option<String>,

  /// An identifier for the user, unique among all Google accounts and never reused.
  /// A Google account can have multiple email addresses at different points in time,
  /// but the sub value is never changed. Use sub within your application as the
  /// unique-identifier key for the user. Maximum length of 255 case-sensitive ASCII
  /// characters.
  pub sub: Option<String>,

  /// The URL of the user's profile picture. Might be provided when:
  ///  - The request scope included the string "profile"
  ///  - The ID token is returned from a token refresh
  /// When picture claims are present, you can use them to update your app's user records.
  /// Note that this claim is never guaranteed to be present.
  pub picture: Option<String>,

  /// The user's locale, represented by a BCP 47 language tag.
  /// Might be provided when a name claim is present.
  pub locale: Option<String>,

  // Other fields are pulled out of the custom payload:

  // /// The audience that this ID token is intended for. It must be one of the
  // /// OAuth 2.0 client IDs of your application.
  // /// NB: You must check that this matches the audience (the app client id)
  // pub aud: Option<String>,

  // /// The Issuer Identifier for the Issuer of the response.
  // /// Always `https://accounts.google.com` or `accounts.google.com` for Google ID tokens.
  // /// NB: You must check that this matches the issuer.
  // pub iss: Option<String>,

  // pub nbf: Option<String>,
  // pub jti: Option<String>,
  // pub iat: Option<String>,
  // pub exp: Option<String>,
}
