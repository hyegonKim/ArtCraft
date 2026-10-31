use crate::error::artcraftx_error::ArtcraftXError;
use core_types::identifiers::credential_id::CredentialId;
use grok_consumer_client::endpoint_bindings::generate_image_websocket::grok_generate_image_websocket::GrokImageWebsocket;
use grok_consumer_client::error::grok_error::GrokError;
use log::{error, info};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// App-wide store of live Grok "imagine" image websockets, one per Grok
/// account (credential).
///
/// The enqueue command opens (or reuses) the socket for the credential it
/// generates with and hands the router a handle to send the prompt on; the
/// third-party task polling thread reads every socket for finished images.
/// [`GrokImageWebsocket`] is itself a cheap, thread-safe handle, so this only
/// guards the "which socket belongs to which account" map.
#[derive(Clone, Default)]
pub struct GrokWebsockets {
  websockets: Arc<RwLock<HashMap<CredentialId, GrokImageWebsocket>>>,
}

impl GrokWebsockets {
  pub fn new() -> Self {
    Self::default()
  }

  /// The account's live socket, connecting (and storing) one if absent.
  pub async fn get_or_connect(
    &self,
    credential_id: &CredentialId,
    cookie_header: &str,
  ) -> Result<GrokImageWebsocket, ArtcraftXError> {
    if let Some(websocket) = self.get(credential_id)? {
      return Ok(websocket);
    }

    info!("Opening Grok image websocket for credential {}", credential_id.as_str());
    let websocket = GrokImageWebsocket::connect(cookie_header)
        .await
        .map_err(|err: GrokError| ArtcraftXError::from(err))?;

    // NB: two concurrent first enqueues for one account can race here; the
    // later insert wins and the earlier socket is dropped (its handle stays
    // valid for the caller that opened it).
    self.insert(credential_id.clone(), websocket.clone())?;
    Ok(websocket)
  }

  pub fn get(&self, credential_id: &CredentialId) -> Result<Option<GrokImageWebsocket>, ArtcraftXError> {
    let websockets = self.websockets.read().map_err(|err| {
      error!("Error reading Grok websockets: {}", err);
      ArtcraftXError::RwLockReadError
    })?;
    Ok(websockets.get(credential_id).cloned())
  }

  pub fn insert(&self, credential_id: CredentialId, websocket: GrokImageWebsocket) -> Result<(), ArtcraftXError> {
    let mut websockets = self.write()?;
    websockets.insert(credential_id, websocket);
    Ok(())
  }

  pub fn remove(&self, credential_id: &CredentialId) -> Result<Option<GrokImageWebsocket>, ArtcraftXError> {
    let mut websockets = self.write()?;
    Ok(websockets.remove(credential_id))
  }

  /// Snapshot of every live socket (handles are cheap clones).
  pub fn all(&self) -> Result<Vec<(CredentialId, GrokImageWebsocket)>, ArtcraftXError> {
    let websockets = self.websockets.read().map_err(|err| {
      error!("Error reading Grok websockets: {}", err);
      ArtcraftXError::RwLockReadError
    })?;
    Ok(websockets.iter().map(|(id, ws)| (id.clone(), ws.clone())).collect())
  }

  pub fn is_empty(&self) -> Result<bool, ArtcraftXError> {
    Ok(self.all()?.is_empty())
  }

  fn write(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<CredentialId, GrokImageWebsocket>>, ArtcraftXError> {
    self.websockets.write().map_err(|err| {
      error!("Error writing Grok websockets: {}", err);
      ArtcraftXError::RwLockWriteError
    })
  }
}
