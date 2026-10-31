use wreq::RequestBuilder;

/// Per-request header secrets captured from a real browser session:
/// the statsig signature plus the tracing ids Grok's web app sends.
///
/// Every field is optional so a partial set still applies. Currently used to
/// replay captured values in live-wire tests; a future change will generate
/// `statsig_id` from fresh cookies.
///
/// NB: `xai_request_id` is sent as the `x-xai-request-id` header.
#[derive(Clone, Debug, Default)]
pub struct GrokRequestHeaders {
  pub statsig_id: Option<String>,
  pub xai_request_id: Option<String>,
  pub traceparent: Option<String>,
  pub sentry_trace: Option<String>,
}

impl GrokRequestHeaders {
  /// Attach whichever headers are present to a request builder.
  pub fn apply(&self, mut builder: RequestBuilder) -> RequestBuilder {
    if let Some(statsig_id) = &self.statsig_id {
      builder = builder.header("x-statsig-id", statsig_id);
    }
    if let Some(xai_request_id) = &self.xai_request_id {
      builder = builder.header("x-xai-request-id", xai_request_id);
    }
    if let Some(traceparent) = &self.traceparent {
      builder = builder.header("traceparent", traceparent);
    }
    if let Some(sentry_trace) = &self.sentry_trace {
      builder = builder.header("sentry-trace", sentry_trace);
    }
    builder
  }
}
