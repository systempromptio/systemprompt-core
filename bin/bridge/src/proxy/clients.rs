//! The reqwest clients the loopback proxy forwards and subscribes through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

// Why: the WSL localhost relay on Windows silently discards an idle keep-alive
// socket; a request written to one afterwards is retransmitted for ~30 s and
// then aborted, which the host app reads as the server being unreachable.
// Reconnecting to a loopback gateway is cheap, so idle sockets are dropped well
// before the relay does it for us. The replay in `forward` covers the window
// this cannot.
const UPSTREAM_POOL_IDLE: Duration = Duration::from_secs(15);

pub(super) fn build_upstream_client() -> std::io::Result<reqwest::Client> {
    reqwest::Client::builder()
        .dns_resolver(Arc::new(crate::gateway::Ipv4FirstResolver))
        .pool_max_idle_per_host(16)
        .pool_idle_timeout(UPSTREAM_POOL_IDLE)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_mins(10))
        .build()
        .map_err(|source| {
            std::io::Error::other(ClientBuildError {
                which: "upstream",
                source,
            })
        })
}

// Why: the comms subscription is one SSE response held open for as long as the
// gateway keeps it; a client-wide total timeout would cut it on a schedule and
// reset the reconnect backoff each time. Only the connect is bounded.
pub(super) fn build_stream_client() -> std::io::Result<reqwest::Client> {
    reqwest::Client::builder()
        .dns_resolver(Arc::new(crate::gateway::Ipv4FirstResolver))
        .pool_max_idle_per_host(1)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|source| {
            std::io::Error::other(ClientBuildError {
                which: "stream",
                source,
            })
        })
}

#[derive(Debug, thiserror::Error)]
#[error("{which} client build failed: {source}")]
struct ClientBuildError {
    which: &'static str,
    #[source]
    source: reqwest::Error,
}
