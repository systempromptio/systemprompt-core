//! Shared bounded `reqwest::Client` builder for AI provider drivers.
//!
//! Every provider HTTP client is built here so no outbound request can hang
//! indefinitely: a request timeout and a connect timeout are always applied.
//! The resilience decorator applies its own per-attempt timeout on top of this;
//! the client-level bound is defence in depth and covers the streaming connect
//! phase.

use std::time::Duration;

use reqwest::Client;

/// Build a `reqwest::Client` with a bounded request and connect timeout.
///
/// A timeout-only configuration cannot make the builder fail for any reason
/// other than the TLS backend failing to initialise — and `Client::new()`
/// panics on that same condition. The fallback therefore only ever runs in a
/// process that is already doomed; it logs the builder error first so the
/// failure is diagnosable rather than silent.
#[must_use]
pub fn build_client(request_timeout: Duration, connect_timeout: Duration) -> Client {
    Client::builder()
        .timeout(request_timeout)
        .connect_timeout(connect_timeout)
        .build()
        .unwrap_or_else(|err| {
            tracing::error!(%err, "reqwest client builder failed; falling back to default client");
            Client::new()
        })
}
