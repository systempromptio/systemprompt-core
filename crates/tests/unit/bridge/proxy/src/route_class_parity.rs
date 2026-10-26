//! The loopback proxy classifies a route once: `classify_route` decides which
//! credential a request may present, and `resolve_route` maps that same class
//! to an upstream. The two must never disagree about what a path is, which is
//! how `/mcp` and `/mcp/` once came apart.

use std::sync::Arc;

use http::Uri;
use systemprompt_bridge::mcp_registry::{McpRegistrySlot, empty_slot};
use systemprompt_bridge::proxy::credential::{RouteClass, classify_path, classify_route};
use systemprompt_bridge::proxy::forward::route::{RouteResolution, resolve_route};
use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::ApiPaths;

fn gateway() -> ValidatedUrl {
    ValidatedUrl::try_new("https://gateway.example.test").expect("valid gateway url")
}

fn uri(s: &str) -> Uri {
    s.parse().expect("uri")
}

fn resolve(path: &str, slot: &McpRegistrySlot) -> RouteResolution {
    resolve_route(&uri(path), &gateway(), slot)
}

#[test]
fn mcp_family_membership_is_decided_once() {
    let slot: Arc<McpRegistrySlot> = empty_slot();
    for path in ["/mcp", "/mcp/", "/mcp/primary", "/mcp/primary/messages"] {
        assert_eq!(classify_path(path), RouteClass::Mcp, "{path}");
        assert!(ApiPaths::is_mcp(path), "{path}");
        assert!(
            !matches!(resolve(path, &slot), RouteResolution::Gateway(_)),
            "{path} is an MCP route and is never forwarded to the gateway as inference"
        );
    }
    for path in ["/mcpx", "/mcpx/primary", "/v1/mcp"] {
        assert_ne!(classify_path(path), RouteClass::Mcp, "{path}");
        assert!(!ApiPaths::is_mcp(path), "{path}");
        assert!(
            matches!(resolve(path, &slot), RouteResolution::Gateway(_)),
            "{path} merely shares a prefix with /mcp"
        );
    }
    assert_eq!(
        ApiPaths::mcp_server_slug("/mcp/primary/messages"),
        Some("primary")
    );
    assert_eq!(ApiPaths::mcp_server_slug("/mcp/"), None);
    assert_eq!(ApiPaths::mcp_server_slug("/mcp"), None);
    assert!(
        matches!(resolve("/mcp", &slot), RouteResolution::UnknownMcp(name) if name.is_empty()),
        "an MCP route with no server slug names no server"
    );
}

#[test]
fn otel_family_is_rewritten_under_the_gateway_surface() {
    let slot: Arc<McpRegistrySlot> = empty_slot();
    for (path, expected) in [
        ("/otel", "https://gateway.example.test/v1/otel"),
        (
            "/otel/v1/traces",
            "https://gateway.example.test/v1/otel/v1/traces",
        ),
        ("/otel?x=1", "https://gateway.example.test/v1/otel?x=1"),
    ] {
        assert_eq!(classify_route(&uri(path)), RouteClass::Otel, "{path}");
        match resolve(path, &slot) {
            RouteResolution::Gateway(url) => assert_eq!(url, expected, "{path}"),
            other => panic!("{path} resolved to {other:?}"),
        }
    }
    assert_ne!(classify_path("/otelx"), RouteClass::Otel);
    assert!(matches!(
        resolve("/otelx", &slot),
        RouteResolution::Gateway(url) if url == "https://gateway.example.test/otelx"
    ));
}

#[test]
fn hook_routes_need_a_plugin_id_to_resolve_as_a_hook() {
    let slot: Arc<McpRegistrySlot> = empty_slot();
    let with_plugin = "/api/public/hooks/track?plugin_id=acme-governance";
    assert!(matches!(
        classify_route(&uri(with_plugin)),
        RouteClass::Hook(Some(id)) if id.as_str() == "acme-governance"
    ));
    assert!(matches!(
        resolve(with_plugin, &slot),
        RouteResolution::Hook { plugin_id, .. } if plugin_id.as_str() == "acme-governance"
    ));
    let without = "/api/public/hooks/track";
    assert_eq!(classify_route(&uri(without)), RouteClass::Hook(None));
    assert!(
        matches!(resolve(without, &slot), RouteResolution::Gateway(_)),
        "a hook route with no plugin id is forwarded for the gateway to refuse"
    );
    assert_ne!(classify_path("/api/public/hooks"), RouteClass::Hook(None));
    assert_ne!(classify_path("/api/public/hooksx"), RouteClass::Hook(None));
}

#[test]
fn inference_excludes_the_bridges_own_gateway_calls() {
    assert_eq!(classify_path("/v1/messages"), RouteClass::Inference);
    assert_eq!(classify_path("/v1/chat/completions"), RouteClass::Inference);
    assert_eq!(classify_path(ApiPaths::BRIDGE_MANIFEST), RouteClass::Other);
    assert_eq!(classify_path(ApiPaths::BRIDGE_BASE), RouteClass::Other);
    assert_eq!(classify_path("/healthz"), RouteClass::Other);
}
