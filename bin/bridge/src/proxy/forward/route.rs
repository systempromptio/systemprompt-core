//! Request routing: the one route classifier decides what a loopback request
//! is, and this module resolves that class to an upstream (gateway, managed
//! MCP server, or plugin hook).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::ApiPaths;

use super::path::{ProxyPath, UpstreamUrlError};
use crate::mcp_registry::{self, McpRegistrySlot, McpUpstream};
use crate::proxy::credential::{RouteClass, classify_route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub url: url::Url,
    pub extra_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResolution {
    Unavailable(String),
    Gateway(url::Url),
    Mcp(Route),
    UnknownMcp(String),
    Hook {
        url: url::Url,
        plugin_id: systemprompt_identifiers::PluginId,
    },
}

impl From<UpstreamUrlError> for RouteResolution {
    fn from(error: UpstreamUrlError) -> Self {
        Self::Unavailable(error.to_string())
    }
}

pub fn resolve_route(
    path: &ProxyPath,
    gateway_base: &ValidatedUrl,
    registry: &McpRegistrySlot,
) -> RouteResolution {
    let class = classify_route(path);
    if class == RouteClass::Mcp {
        return resolve_mcp(path.path(), gateway_base, registry);
    }
    let url = match path.gateway_url(gateway_base) {
        Ok(url) => url,
        Err(error) => return error.into(),
    };
    match class {
        RouteClass::Hook(Some(plugin_id)) => RouteResolution::Hook { url, plugin_id },
        RouteClass::Mcp
        | RouteClass::Hook(None)
        | RouteClass::Inference
        | RouteClass::Otel
        | RouteClass::Other => RouteResolution::Gateway(url),
    }
}

fn resolve_mcp(
    path: &str,
    gateway_base: &ValidatedUrl,
    registry: &McpRegistrySlot,
) -> RouteResolution {
    let Some(name) = ApiPaths::mcp_server_slug(path) else {
        return RouteResolution::UnknownMcp(String::new());
    };
    if let Some(entry) = mcp_registry::snapshot(registry).get(name) {
        return mcp_route(entry);
    }
    if let Err(e) = mcp_registry::rehydrate_from_disk(registry, gateway_base) {
        return RouteResolution::Unavailable(e.to_string());
    }
    mcp_registry::snapshot(registry)
        .get(name)
        .map_or_else(|| RouteResolution::UnknownMcp(name.to_owned()), mcp_route)
}

fn mcp_route(entry: &McpUpstream) -> RouteResolution {
    match url::Url::parse(entry.url.as_str()) {
        Ok(url) => RouteResolution::Mcp(Route {
            url,
            extra_headers: entry.headers.clone(),
        }),
        Err(error) => RouteResolution::Unavailable(error.to_string()),
    }
}

pub(super) fn same_origin_as(upstream: &url::Url, gateway_base: &ValidatedUrl) -> bool {
    url::Url::parse(gateway_base.as_str()).is_ok_and(|base| upstream.origin() == base.origin())
}
