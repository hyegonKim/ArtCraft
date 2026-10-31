use grok_consumer_client::endpoint_bindings::generate_image_websocket::grok_generate_image_websocket::GrokImageWebsocket;

/// The first-party (cookie-session) Grok client used by the router.
///
/// Drives the user's own logged-in grok.com session directly via captured
/// cookies (distinct from the API-key `RouterGrokApiClient`). Image generation
/// runs over Grok's "imagine" websocket — the app owns one live socket per
/// account and hands the router a handle; the router only *sends* prompts on
/// it (the app's polling thread reads the results). Video generation
/// additionally needs an `x-statsig-id` signature.
// NB: `cookie_header` / `maybe_statsig` are carried for the (HTTP) video path;
// the image path only uses the websocket.
#[allow(dead_code)]
pub struct RouterGrokClient {
  pub(crate) cookie_header: String,

  /// A browser-minted `x-statsig-id` for `POST /rest/app-chat/conversations/new`
  /// (video only). `None` when unavailable — image generation ignores it.
  pub(crate) maybe_statsig: Option<String>,

  /// The account's live imagine websocket. A cheap, shareable handle.
  pub(crate) image_websocket: GrokImageWebsocket,
}

impl RouterGrokClient {
  pub fn new(cookie_header: String, maybe_statsig: Option<String>, image_websocket: GrokImageWebsocket) -> Self {
    Self { cookie_header, maybe_statsig, image_websocket }
  }
}
