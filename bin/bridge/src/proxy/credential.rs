//! Which credential a loopback caller presented, judged per route class.
//!
//! The raw loopback secret lives only in bridge-owned 0600 files, so a route
//! whose credential is read from a surface other local accounts can see
//! (a plugin's `hooks.json`, Claude Desktop's managed preferences) accepts
//! only the token derived for that surface. Each class is fail-closed: a
//! credential that does not fit the route is refused, never downgraded to a
//! raw-secret check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::{HostId, PluginId, ProxySecret};
use crate::proxy::scoped_token::{self, TokenScope};
use crate::proxy::secret;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopbackCredential {
    Secret,
    Hook(PluginId),
    Host(HostId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteClass {
    Hook(Option<PluginId>),
    Inference,
    Mcp,
    Otel,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    NoCredential,
    SecretMismatch,
    ScopeMismatch,
}

impl Rejection {
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NoCredential => "no-credential",
            Self::SecretMismatch => "secret-mismatch",
            Self::ScopeMismatch => "scope-mismatch",
        }
    }

    #[must_use]
    pub const fn status(self) -> http::StatusCode {
        match self {
            Self::NoCredential | Self::SecretMismatch => http::StatusCode::FORBIDDEN,
            Self::ScopeMismatch => http::StatusCode::UNAUTHORIZED,
        }
    }
}

#[must_use]
pub fn classify_route(uri: &http::Uri) -> RouteClass {
    let path = uri.path();
    if path.starts_with("/api/public/hooks/") {
        return RouteClass::Hook(hook_plugin_id(uri));
    }
    if path == "/otel" || path.starts_with("/otel/") {
        return RouteClass::Otel;
    }
    if path == "/mcp" || path.starts_with("/mcp/") {
        return RouteClass::Mcp;
    }
    if path.starts_with("/v1/") && !path.starts_with("/v1/bridge/") {
        return RouteClass::Inference;
    }
    RouteClass::Other
}

fn hook_plugin_id(uri: &http::Uri) -> Option<PluginId> {
    uri.query()?.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == "plugin_id").then(|| PluginId::try_new(v).ok())?
    })
}

pub fn authenticate(
    presented: &str,
    secret: &ProxySecret,
    route: &RouteClass,
) -> Result<LoopbackCredential, Rejection> {
    if presented.is_empty() {
        return Err(Rejection::NoCredential);
    }
    let is_secret = secret::verify(presented, secret);
    let host = presenting_host(presented, secret);
    match route {
        RouteClass::Hook(None) => Err(Rejection::ScopeMismatch),
        RouteClass::Hook(Some(plugin)) => {
            if scoped_token::verify(presented, secret, &TokenScope::Hook(plugin.clone())) {
                Ok(LoopbackCredential::Hook(plugin.clone()))
            } else {
                Err(Rejection::ScopeMismatch)
            }
        },
        // Why: a host's profile is a file other local accounts can read
        // (Claude Desktop's managed preferences carry `inferenceGatewayApiKey`
        // because its third-party gateway contract has no credential helper),
        // so the inference and managed-MCP routes a host drives accept the
        // token derived for that host rather than the secret itself.
        RouteClass::Inference | RouteClass::Mcp => {
            if is_secret {
                Ok(LoopbackCredential::Secret)
            } else if let Some(host) = host {
                Ok(LoopbackCredential::Host(host))
            } else {
                Err(Rejection::SecretMismatch)
            }
        },
        RouteClass::Otel | RouteClass::Other => {
            if is_secret {
                Ok(LoopbackCredential::Secret)
            } else if host.is_some() {
                Err(Rejection::ScopeMismatch)
            } else {
                Err(Rejection::SecretMismatch)
            }
        },
    }
}

fn presenting_host(presented: &str, secret: &ProxySecret) -> Option<HostId> {
    systemprompt_models::bridge::profile::KNOWN_HOSTS
        .iter()
        .map(|id| HostId::new(*id))
        .find(|host| scoped_token::verify(presented, secret, &TokenScope::Host(host.clone())))
}
