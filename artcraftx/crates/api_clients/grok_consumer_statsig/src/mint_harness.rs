/// JavaScript injected into a loaded grok.com WebView to harvest `x-statsig-id`
/// signatures.
///
/// # How it works
///
/// Rather than re-implement or call Grok's (obfuscated, rotating) signer, the
/// harness *observes*: it wraps `fetch` and `XMLHttpRequest` and, for every
/// outgoing request that already carries an `x-statsig-id` header, reports the
/// tuple `{ method, path, statsigId, capturedAt }` back to the host. The header
/// is produced by Grok's own code inside its own DOM, so it is always current
/// and valid — the harness never derives anything.
///
/// The page emits a `POST /rest/app-chat/conversations/new` signature whenever
/// it starts a chat or generation, so the app's oracle keeps a hidden grok.com
/// WebView warm and (for a cold cache) nudges it to begin — then immediately
/// abort — a trivial conversation to force that request. Each captured
/// signature is short-lived (see [`MintedStatsig`](crate::MintedStatsig)), so
/// the host caches the most recent one per endpoint.
///
/// # Host contract
///
/// The harness delivers each capture by calling, in preference order:
///   1. `window.__grokStatsigReport(jsonString)` — define this from a Tauri
///      init script that forwards to a command / channel; **recommended**.
///   2. `window.ipc.postMessage(jsonString)` — the wry IPC fallback.
///
/// The `jsonString` is `{"method","path","statsigId","capturedAt"}` where
/// `capturedAt` is browser `Date.now()` in ms. Installing the harness twice is
/// a no-op (it guards on a global flag), so it is safe to re-inject after a
/// document swap.
pub const MINT_HARNESS_SCRIPT: &str = r#"
(function () {
  if (window.__grokStatsigHarnessInstalled) return;
  window.__grokStatsigHarnessInstalled = true;

  var HEADER = "x-statsig-id";

  function report(method, url, statsigId) {
    if (!statsigId) return;
    var path;
    try { path = new URL(url, location.href).pathname; } catch (e) { path = String(url); }
    var payload = JSON.stringify({
      method: String(method || "GET").toUpperCase(),
      path: path,
      statsigId: statsigId,
      capturedAt: Date.now()
    });
    try {
      if (typeof window.__grokStatsigReport === "function") {
        window.__grokStatsigReport(payload);
      } else if (window.ipc && typeof window.ipc.postMessage === "function") {
        window.ipc.postMessage(payload);
      }
    } catch (e) { /* never let reporting break the page's own request */ }
  }

  // Read a header value out of whatever init.headers shape fetch() was given.
  function headerFrom(init) {
    if (!init || !init.headers) return null;
    var h = init.headers;
    try {
      if (typeof Headers !== "undefined" && h instanceof Headers) return h.get(HEADER);
      if (Array.isArray(h)) {
        for (var i = 0; i < h.length; i++) {
          if (String(h[i][0]).toLowerCase() === HEADER) return h[i][1];
        }
        return null;
      }
      for (var k in h) {
        if (Object.prototype.hasOwnProperty.call(h, k) && k.toLowerCase() === HEADER) return h[k];
      }
    } catch (e) { /* fall through */ }
    return null;
  }

  var origFetch = window.fetch;
  if (typeof origFetch === "function") {
    window.fetch = function (input, init) {
      try {
        var method = (init && init.method) || (input && input.method) || "GET";
        var url = (input && input.url) || input;
        var statsig = headerFrom(init);
        // A Request object may carry the header on itself.
        if (!statsig && input && input.headers && typeof input.headers.get === "function") {
          statsig = input.headers.get(HEADER);
        }
        report(method, url, statsig);
      } catch (e) { /* observation must never break fetch */ }
      return origFetch.apply(this, arguments);
    };
  }

  var OrigXHR = window.XMLHttpRequest;
  if (typeof OrigXHR === "function") {
    var open = OrigXHR.prototype.open;
    var setHeader = OrigXHR.prototype.setRequestHeader;
    var send = OrigXHR.prototype.send;
    OrigXHR.prototype.open = function (method, url) {
      this.__statsigMethod = method;
      this.__statsigUrl = url;
      return open.apply(this, arguments);
    };
    OrigXHR.prototype.setRequestHeader = function (name, value) {
      if (String(name).toLowerCase() === HEADER) this.__statsigId = value;
      return setHeader.apply(this, arguments);
    };
    OrigXHR.prototype.send = function () {
      try { report(this.__statsigMethod, this.__statsigUrl, this.__statsigId); }
      catch (e) { /* ignore */ }
      return send.apply(this, arguments);
    };
  }
})();
"#;
