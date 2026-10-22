//! Request routing: the one route classifier decides what a loopback request
//! is, and this module resolves that class to an upstream (gateway, managed
//! MCP server, or plugin hook).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::ApiPaths;

use crate::mcp_registry::{self, McpRegistrySlot};
use crate::proxy::credential::{RouteClass, classify_route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub url: String,
    pub extra_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResolution {
    Unavailable(String),
    Gateway(String),
    Mcp(Route),
    UnknownMcp(String),
    Hook {
        url: String,
        plugin_id: systemprompt_identifiers::PluginId,
    },
}

pub fn resolve_route(
    uri: &http::Uri,
    gateway_base: &ValidatedUrl,
    registry: &McpRegistrySlot,
) -> RouteResolution {
    match classify_route(uri) {
        RouteClass::Mcp => resolve_mcp(uri.path(), gateway_base, registry),
        RouteClass::Hook(Some(plugin_id)) => RouteResolution::Hook {
            url: build_gateway_url(gateway_base, uri),
            plugin_id,
        },
        RouteClass::Hook(None) | RouteClass::Inference | RouteClass::Otel | RouteClass::Other => {
            RouteResolution::Gateway(build_gateway_url(gateway_base, uri))
        },
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
        return RouteResolution::Mcp(Route {
            url: entry.url.as_str().to_owned(),
            extra_headers: entry.headers.clone(),
        });
    }
    if let Err(e) = mcp_registry::rehydrate_from_disk(registry, gateway_base) {
        return RouteResolution::Unavailable(e.to_string());
    }
    mcp_registry::snapshot(registry).get(name).map_or_else(
        || RouteResolution::UnknownMcp(name.to_owned()),
        |entry| {
            RouteResolution::Mcp(Route {
                url: entry.url.as_str().to_owned(),
                extra_headers: entry.headers.clone(),
            })
        },
    )
}

pub(super) fn same_origin_as(upstream: &str, gateway_base: &ValidatedUrl) -> bool {
    match (
        url::Url::parse(upstream),
        url::Url::parse(gateway_base.as_str()),
    ) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    }
}

fn build_gateway_url(gateway_base: &ValidatedUrl, uri: &http::Uri) -> String {
    let path_and_query = uri.path_and_query().map_or("/", |p| p.as_str());
    let separator = if path_and_query.starts_with('/') {
        ""
    } else {
        "/"
    };
    let rewritten = rewrite_otel_to_v1(path_and_query);
    let path_and_query = rewritten.as_deref().unwrap_or(path_and_query);
    format!(
        "{base}{separator}{path_and_query}",
        base = gateway_base.as_str().trim_end_matches('/'),
    )
}

// Why: the gateway serves OTLP under its `/v1` surface; clients configured
// with the bridge's bare `/otel` endpoint are mapped onto it here.
fn rewrite_otel_to_v1(path_and_query: &str) -> Option<String> {
    let (path, suffix) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(p, q)| (p, Some(q)));
    if !ApiPaths::is_otel(path) {
        return None;
    }
    let base = ApiPaths::GATEWAY_BASE;
    Some(suffix.map_or_else(|| format!("{base}{path}"), |q| format!("{base}{path}?{q}")))
}
