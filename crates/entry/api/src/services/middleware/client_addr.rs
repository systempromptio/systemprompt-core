//! The client address of a whole request, resolved against the profile's
//! trusted proxies through [`systemprompt_security::http::resolve_client_ip`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use systemprompt_manifest::Config;
use systemprompt_security::http::resolve_client_ip;

#[must_use]
pub(crate) fn client_ip_from_request(
    request: &axum::extract::Request,
    config: &Config,
) -> Option<IpAddr> {
    resolve_client_ip(
        request.headers(),
        request.extensions().get::<ConnectInfo<SocketAddr>>(),
        &config.trusted_proxies,
    )
}
