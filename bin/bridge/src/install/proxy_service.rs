//! Brings the loopback proxy up before anything writes its origin into a file.
//!
//! `sync` and `install` bake `http://127.0.0.1:<port>` into host profiles,
//! plugin `hooks.json`, and `.mcp.json`. Those files are a promise that
//! something is listening there, and until this module existed the promise was
//! made by a one-shot process and kept by a desktop GUI nobody guarantees is
//! running — which is how a Mac with the app closed ended up refusing every
//! `PreToolUse` hook the agent host fired.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use crate::proxy::peer::{self as proxy_probe, PeerIdentity};

const READY_TIMEOUT: Duration = Duration::from_secs(6);
const READY_POLL: Duration = Duration::from_millis(250);

/// Whether a loopback origin written right now would name a live proxy.
#[derive(Debug, Clone)]
pub enum ProxyReadiness {
    Live(u16),
    // Why: the origin is still written even when nothing answers — clients
    // pick it up once the proxy does come up — so the caller has to say so
    // out loud rather than treat this as success.
    Unavailable { port: u16, reason: String },
}

impl ProxyReadiness {
    #[must_use]
    pub const fn port(&self) -> u16 {
        match self {
            Self::Live(port) | Self::Unavailable { port, .. } => *port,
        }
    }

    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self, Self::Live(_))
    }
}

#[must_use]
pub fn ensure_running() -> ProxyReadiness {
    let port = crate::proxy::resolved_port();
    if matches!(proxy_probe::probe_identity(port), PeerIdentity::Ours(_)) {
        return ProxyReadiness::Live(port);
    }

    let binary = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            return ProxyReadiness::Unavailable {
                port,
                reason: format!("cannot resolve this binary's path: {e}"),
            };
        },
    };

    match super::schedule_apply::ensure_proxy_job(&binary) {
        Ok(true) => {
            tracing::info!(
                target: "bridge::proxy",
                port,
                "no proxy was listening; started the supervised one"
            );
            wait_for_ready(port)
        },
        Ok(false) => ProxyReadiness::Unavailable {
            port,
            reason: format!(
                "the {} desktop app owns the proxy on this platform and is not running",
                crate::brand::brand().app_name
            ),
        },
        Err(e) => ProxyReadiness::Unavailable {
            port,
            reason: format!("could not register the proxy supervisor: {e}"),
        },
    }
}

// Why: the supervisor can bind a different port than the one this process
// resolved before it existed, so the portfile is re-read on every poll.
fn wait_for_ready(resolved: u16) -> ProxyReadiness {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let candidate = crate::proxy::portfile::read().map_or(resolved, |r| r.port);
        if matches!(
            proxy_probe::probe_identity(candidate),
            PeerIdentity::Ours(_)
        ) {
            crate::proxy::set_resolved_port(candidate);
            return ProxyReadiness::Live(candidate);
        }
        if Instant::now() >= deadline {
            return ProxyReadiness::Unavailable {
                port: candidate,
                reason: "the proxy supervisor was started but did not begin listening".to_owned(),
            };
        }
        std::thread::sleep(READY_POLL);
    }
}

pub fn ensure_running_reported() -> ProxyReadiness {
    let readiness = ensure_running();
    if let ProxyReadiness::Unavailable { port, reason } = &readiness {
        crate::stdio::diag(&format!(
            "proxy: nothing is listening on 127.0.0.1:{port} — {reason}. Client config written \
             now names that port and will be refused until the proxy is up; start it with `{bin} \
             proxy` or register the supervisor with `{bin} install --apply-schedule`.",
            bin = crate::brand::brand().binary_name
        ));
    }
    readiness
}
