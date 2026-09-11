//! Loopback inference proxy: server, forwarding, token cache, MCP probe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bind;
pub mod comms;
pub mod dispatch;
pub mod forward;
pub mod handle;
pub mod heartbeat;
pub mod identity;
pub mod keepalive;
pub mod loopback;
pub mod mcp_probe;
pub mod peer;
pub mod portfile;
mod refresh;
pub mod secret;
pub mod server;
pub mod session;
pub mod sign_in_latch;
pub mod token_cache;
pub mod usage;

use std::time::Duration;

use identity::InstallId;

pub use handle::{ProxyDeps, ProxyHandle, ProxyRole};
pub use loopback::LoopbackEndpoint;
pub use server::{DRAIN_DEADLINE, ProxyContext, ProxyStats, ServedProxy};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
pub(crate) const REFRESH_TICK: Duration = Duration::from_mins(1);
pub use forward::REFRESH_THRESHOLD_SECS;

pub const MAX_CANDIDATE_PORT: u16 = DEFAULT_PROXY_PORT + 9;

pub fn candidate_ports(ours: &InstallId) -> std::io::Result<Vec<u16>> {
    Ok(candidate_ports_from(portfile::preferred_port(ours)?))
}

#[must_use]
pub fn candidate_ports_from(preferred: Option<u16>) -> Vec<u16> {
    let mut ports = Vec::with_capacity(11);
    if let Some(preferred) = preferred {
        ports.push(preferred);
    }
    for p in DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }
    ports
}
