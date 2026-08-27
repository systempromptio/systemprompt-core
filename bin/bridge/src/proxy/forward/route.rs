//! Request routing: gateway, managed MCP, and plugin hook URL resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::ValidatedUrl;

use crate::mcp_registry;

pub(super) struct Route {
    pub url: String,
    pub extra_headers: BTreeMap<String, String>,
}

pub(super) enum RouteResolution {
    Gateway(String),
    Mcp(Route),
    UnknownMcp(String),
    Hook {
        url: String,
        plugin_id: systemprompt_identifiers::PluginId,
    },
}

pub(super) fn resolve_route(uri: &http::Uri, gateway_base: &ValidatedUrl) -> RouteResolution {
    if let Some(name) = parse_mcp_path(uri.path()) {
        if let Some(entry) = mcp_registry::snapshot().get(name) {
            return RouteResolution::Mcp(Route {
                url: entry.url.as_str().to_owned(),
                extra_headers: entry.headers.clone(),
            });
        }
        // Why: on a fresh install the proxy can start before the first sync
        // writes mcp-servers.json, leaving the boot-time rehydrate empty and
        // every /mcp/<name> a 404 for the life of the process. A miss re-reads
        // the fragment once before answering — the sync process publishes into
        // its own memory, not this one's.
        mcp_registry::rehydrate_from_disk();
        return mcp_registry::snapshot().get(name).map_or_else(
            || RouteResolution::UnknownMcp(name.to_owned()),
            |entry| {
                RouteResolution::Mcp(Route {
                    url: entry.url.as_str().to_owned(),
                    extra_headers: entry.headers.clone(),
                })
            },
        );
    }
    if uri.path().starts_with("/api/public/hooks/")
        && let Some(plugin_id) = parse_hook_plugin_id(uri)
    {
        return RouteResolution::Hook {
            url: build_gateway_url(gateway_base, uri),
            plugin_id,
        };
    }
    RouteResolution::Gateway(build_gateway_url(gateway_base, uri))
}

fn parse_mcp_path(path: &str) -> Option<&str> {
    let stripped = path.strip_prefix("/mcp/")?;
    let name = stripped.split('/').next()?;
    if name.is_empty() { None } else { Some(name) }
}

fn parse_hook_plugin_id(uri: &http::Uri) -> Option<systemprompt_identifiers::PluginId> {
    uri.query()?.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == "plugin_id" && !v.is_empty()).then(|| systemprompt_identifiers::PluginId::new(v))
    })
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

// Why: OTLP exporters POST `/otel` without the `/v1` prefix the gateway router
// is nested under.
fn rewrite_otel_to_v1(path_and_query: &str) -> Option<String> {
    let (path, suffix) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(p, q)| (p, Some(q)));
    if path != "/otel" && !path.starts_with("/otel/") {
        return None;
    }
    Some(suffix.map_or_else(|| format!("/v1{path}"), |q| format!("/v1{path}?{q}")))
}
