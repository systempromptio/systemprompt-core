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

use crate::ids::{PluginId, ProxySecret};
use crate::proxy::scoped_token::{self, TokenScope};
use crate::proxy::secret;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::host::HostKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopbackCredential {
    Secret,
    Hook(PluginId),
    Host(HostKind),
}

impl LoopbackCredential {
    pub const fn verified_host(&self) -> Option<HostKind> {
        match self {
            Self::Host(host) => Some(*host),
            Self::Secret | Self::Hook(_) => None,
        }
    }
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
    match classify_path(uri.path()) {
        RouteClass::Hook(_) => RouteClass::Hook(hook_plugin_id(uri)),
        class => class,
    }
}

// Why: the bridge's own `/v1/bridge/*` calls to the gateway are not inference
// a client can present a credential for; everything else under `/v1/` is.
#[must_use]
pub fn classify_path(path: &str) -> RouteClass {
    if ApiPaths::is_public_hook(path) {
        return RouteClass::Hook(None);
    }
    if ApiPaths::is_otel(path) {
        return RouteClass::Otel;
    }
    if ApiPaths::is_mcp(path) {
        return RouteClass::Mcp;
    }
    let gateway = format!("{}/", ApiPaths::GATEWAY_BASE);
    let bridge = format!("{}/", ApiPaths::BRIDGE_BASE);
    if path.starts_with(&gateway) && path != ApiPaths::BRIDGE_BASE && !path.starts_with(&bridge) {
        return RouteClass::Inference;
    }
    RouteClass::Other
}

pub fn hook_plugin_id(uri: &http::Uri) -> Option<PluginId> {
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
        RouteClass::Inference | RouteClass::Mcp | RouteClass::Otel => {
            if is_secret {
                Ok(LoopbackCredential::Secret)
            } else if let Some(host) = host {
                Ok(LoopbackCredential::Host(host))
            } else {
                Err(Rejection::SecretMismatch)
            }
        },
        RouteClass::Other => {
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

fn presenting_host(presented: &str, secret: &ProxySecret) -> Option<HostKind> {
    HostKind::ALL
        .into_iter()
        .find(|host| scoped_token::verify(presented, secret, &TokenScope::Host(*host)))
}
