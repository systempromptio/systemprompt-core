//! Who is answering on the loopback port: this bridge, another bridge, or
//! something else — the distinction the plain reachability probe cannot make.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::proxy::identity::{InstallId, WhoAmI};
use crate::proxy_probe::{http_get_body, resolve_first_addr};

/// Who is answering on a loopback port.
///
/// The distinction `probe` cannot make: it reports `Listening` for anything
/// that returns a parsable status line, which is how a foreign bridge holding
/// our port has been reading as healthy.
#[derive(Debug, Clone)]
pub enum PeerIdentity {
    Ours(WhoAmI),
    Foreign(WhoAmI),
    Unknown,
    Unreachable,
}

#[must_use]
pub fn probe_identity(port: u16, ours: &InstallId) -> PeerIdentity {
    let addr = format!("127.0.0.1:{port}");
    let Some(resolved) = resolve_first_addr(&addr) else {
        return PeerIdentity::Unreachable;
    };
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&resolved, std::time::Duration::from_millis(1500))
    else {
        return PeerIdentity::Unreachable;
    };
    let Ok(body) = http_get_body(
        &mut stream,
        "127.0.0.1",
        crate::proxy::dispatch::WHOAMI_PATH,
    ) else {
        return PeerIdentity::Unknown;
    };
    _ = stream.shutdown(std::net::Shutdown::Both);

    let Ok(who) = serde_json::from_str::<WhoAmI>(&body) else {
        return PeerIdentity::Unknown;
    };
    if who.product != crate::proxy::identity::WHOAMI_PRODUCT {
        return PeerIdentity::Unknown;
    }
    if !who.install_id.is_known() {
        return PeerIdentity::Unknown;
    }
    if who.is_ours(ours) {
        PeerIdentity::Ours(who)
    } else {
        PeerIdentity::Foreign(who)
    }
}
