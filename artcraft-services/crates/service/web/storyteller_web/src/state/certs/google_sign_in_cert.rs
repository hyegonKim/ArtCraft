use anyhow::anyhow;
use errors::AnyhowResult;
use google_sign_in::certs::download_certs::download_cert_key_set;
use google_sign_in::certs::key_map::KeyMap;
use log::{error, info};
use std::sync::{Arc, RwLock};

/// The Google Sign In cert needs to be refreshed periodically
#[derive(Clone)]
pub struct GoogleSignInCert {
  /// Multiple recent JWK public keys from Google
  key_map: Arc<RwLock<Option<KeyMap>>>
}

impl GoogleSignInCert {
  pub fn new() -> Self {
    Self {
      key_map: Arc::new(RwLock::new(None))
    }
  }

  /// Get the current key map
  /// If available, this returns a copy so that we don't spend time in the critical section.
  pub async fn fetch_key_map(&self, refresh: bool) -> AnyhowResult<KeyMap> {
    if !refresh {
      match self.key_map.read() {
        Err(err) => {
          error!("Error reading lock: {:?}", err);
          return Err(anyhow!("Error reading lock: {:?}", err));
        }
        Ok(read) => {
          if let Some(key_map) = &*read {
            return Ok(key_map.clone());
          }
        }
      }
    }
    info!("Fetching Google Sign In certs...");
    let new_certs = download_cert_key_set().await?;
    let _r = self.set_key_map(new_certs.clone());
    Ok(new_certs)
  }

  fn set_key_map(&self, key_map: KeyMap) -> AnyhowResult<()> {
    match self.key_map.write() {
      Err(err) => {
        error!("Error writing lock: {:?}", err);
        Err(anyhow!("Error writing lock: {:?}", err))
      }
      Ok(mut write) => {
        *write = Some(key_map);
        Ok(())
      }
    }
  }
}
