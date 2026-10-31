-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Google SSO / "Sign in with Google" data
-- Many of the values come from OpenID Connect:
-- https://developers.google.com/identity/openid-connect/openid-connect
CREATE TABLE google_sign_in_accounts(
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  token VARCHAR(32) NOT NULL,

  -- An identifier for the user, unique among all Google accounts and never reused.
  -- A Google account can have multiple email addresses at different points in time, but the sub
  -- value is never changed. Use sub within your application as the unique-identifier key for the
  -- user. Maximum length of 255 case-sensitive ASCII characters.
  --
  -- https://developers.google.com/identity/openid-connect/openid-connect#an-id-tokens-payload
  --
  -- Subject Identifier. A locally unique and never reassigned identifier within
  -- the Issuer for the End-User, which is intended to be consumed by the Client, e.g., 24400320
  -- or AItOawmwtWwcT0k51BayewNvutrJUqsvl6qs7A4. It MUST NOT exceed 255 ASCII characters in
  -- length. The sub value is a case sensitive string."
  --
  -- https://stackoverflow.com/a/72091170
  -- https://stackoverflow.com/a/74003331
  --
  subject VARCHAR(255) NOT NULL,

  -- ========== FOREIGN KEYS ==========

  -- Foreign key link to the users table record.
  -- This could be removed later if a user disassociates their account.
  maybe_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== OTHER INFO ==========

  -- The user's email address. Provided only if you included the email scope in your request.
  -- The value of this claim may not be unique to this account and could change over time,
  -- therefore you should not use this value as the primary identifier to link to your user record.
  -- You also can't rely on the domain of the email claim to identify users of Google Workspace or
  -- Cloud organizations; use the hd claim instead.
  --
  -- NB: If this is null, we'll simply deny the sign in / sign up event. So effectively, our copy
  -- will never be null.
  email_address VARCHAR(255) NOT NULL,

  -- True if the user's e-mail address has been verified; otherwise false.
  is_email_verified BOOLEAN NOT NULL DEFAULT false,

  -- The user's locale, represented by a BCP 47 language tag.
  -- Might be provided when a name claim is present.
  maybe_locale VARCHAR(255) DEFAULT NULL,

  -- The user's full name, in a displayable form. Might be provided when: [...]
  maybe_name VARCHAR(255) DEFAULT NULL,

  -- The user's given name(s) or first name(s). Might be provided when a name claim is present.
  maybe_given_name VARCHAR(255) DEFAULT NULL,

  -- The user's surname(s) or last name(s). Might be provided when a name claim is present.
  maybe_family_name VARCHAR(255) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_update VARCHAR(40) NOT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (subject),
  KEY index_maybe_user_token (maybe_user_token),
  KEY index_email_address (email_address),
  KEY index_created_at (created_at),
  KEY index_updated_at (updated_at)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
