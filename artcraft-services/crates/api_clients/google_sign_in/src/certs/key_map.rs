use std::collections::HashMap;
use jwt_simple::algorithms::RS256PublicKey;

/// Map of JWK `kid` to `RS256PublicKey`.
/// (JWK sets typically contain more than one key.)
pub type KeyMap = HashMap<String, RS256PublicKey>;
