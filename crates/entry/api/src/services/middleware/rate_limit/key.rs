//! The in-process governor's key: verified identity where one is available,
//! resolved client address otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{ConnectInfo, Request};
use ipnet::IpNet;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;

use crate::services::middleware::client_addr::{forwarded_headers_ignored, resolve_client_ip};

const UNTRUSTED_PROXY_WARN_INTERVAL_SECS: u64 = 300;

/// Buckets a request by authenticated identity where one is available, and by
/// resolved client address otherwise.
///
/// The identity branch only fires on routers that carry `with_auth`, because
/// that is what puts a [`RequestContext`] in the request's extensions. A router
/// rate-limited without it — the gateway is one, since it authenticates inside
/// each handler instead — buckets every caller by address. That is sound, but
/// it means a whole office behind one NAT shares a budget, so those routers
/// need a budget sized for a network rather than for a person.
///
/// It warns when a proxy is in the path but is not trusted, because every
/// client behind it then collapses onto a single bucket. That is the failure
/// that hides itself: the limiter behaves exactly as configured, the instance
/// simply runs out of budget for everyone at once and refuses requests that
/// look like credential failures to the caller.
#[derive(Clone, Debug)]
pub struct IdentityOrTrustedIpKey {
    trusted_proxies: Arc<Vec<IpNet>>,
    untrusted_proxy_warn: Arc<systemprompt_logging::LogThrottle>,
}

impl IdentityOrTrustedIpKey {
    pub(super) fn new(trusted_proxies: Arc<Vec<IpNet>>) -> Self {
        Self {
            trusted_proxies,
            untrusted_proxy_warn: Arc::new(systemprompt_logging::LogThrottle::new(
                UNTRUSTED_PROXY_WARN_INTERVAL_SECS,
            )),
        }
    }

    fn warn_if_proxy_untrusted<T>(&self, req: &Request<T>) {
        let Some(peer) = req.extensions().get::<ConnectInfo<SocketAddr>>() else {
            return;
        };
        if forwarded_headers_ignored(req.headers(), peer.0.ip(), &self.trusted_proxies)
            && self.untrusted_proxy_warn.allow()
        {
            tracing::warn!(
                peer_ip = %peer.0.ip(),
                "rate limiting by proxy address: this request carried forwarded client-IP \
                 headers but the peer is absent from server.trusted_proxies, so every client \
                 behind that proxy shares one rate-limit bucket and will exhaust it together; \
                 add the peer's range to server.trusted_proxies"
            );
        }
    }
}

impl tower_governor::key_extractor::KeyExtractor for IdentityOrTrustedIpKey {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, tower_governor::GovernorError> {
        if let Some(ctx) = req.extensions().get::<RequestContext>()
            && ctx.auth.user_type != UserType::Anon
        {
            return Ok(format!("u:{}", ctx.user_id()));
        }

        self.warn_if_proxy_untrusted(req);

        resolve_client_ip(
            req.headers(),
            req.extensions().get::<ConnectInfo<SocketAddr>>(),
            &self.trusted_proxies,
        )
        .map(|ip| format!("ip:{ip}"))
        .ok_or(tower_governor::GovernorError::UnableToExtractKey)
    }
}
