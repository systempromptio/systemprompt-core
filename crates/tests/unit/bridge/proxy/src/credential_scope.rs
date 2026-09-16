//! Route-class credential matrix for the loopback proxy: the raw secret, a
//! per-plugin hook token and a per-host token each open exactly the routes
//! their surface is read from, and nothing else.

use http::Uri;
use systemprompt_bridge::ids::{HostId, LoopbackSecret, PluginId, ProxySecret};
use systemprompt_bridge::proxy::credential::{
    LoopbackCredential, Rejection, RouteClass, authenticate, classify_route,
};
use systemprompt_bridge::proxy::scoped_token::{hook_token, host_token};

const SECRET: &str = "loopback-secret-0123456789abcdef0123456789abcdef";

fn secret() -> ProxySecret {
    ProxySecret::new(SECRET)
}

fn plugin(id: &str) -> PluginId {
    PluginId::try_new(id).expect("plugin id")
}

fn hook(id: &str) -> String {
    hook_token(&LoopbackSecret::new(SECRET), &plugin(id))
        .as_str()
        .to_owned()
}

fn host(id: &str) -> String {
    host_token(&LoopbackSecret::new(SECRET), &HostId::new(id))
        .as_str()
        .to_owned()
}

fn uri(s: &str) -> Uri {
    s.parse().expect("uri")
}

#[test]
fn routes_are_classified_by_the_surface_their_credential_is_read_from() {
    assert_eq!(
        classify_route(&uri("/api/public/hooks/govern?plugin_id=acme")),
        RouteClass::Hook(Some(plugin("acme")))
    );
    assert_eq!(
        classify_route(&uri("/api/public/hooks/track")),
        RouteClass::Hook(None)
    );
    assert_eq!(classify_route(&uri("/v1/messages")), RouteClass::Inference);
    assert_eq!(
        classify_route(&uri("/v1/chat/completions")),
        RouteClass::Inference
    );
    assert_eq!(classify_route(&uri("/mcp/x")), RouteClass::Mcp);
    assert_eq!(classify_route(&uri("/mcp")), RouteClass::Mcp);
    assert_eq!(classify_route(&uri("/otel")), RouteClass::Otel);
    assert_eq!(classify_route(&uri("/otel/v1/traces")), RouteClass::Otel);
    assert_eq!(
        classify_route(&uri("/v1/bridge/manifest")),
        RouteClass::Other
    );
    assert_eq!(classify_route(&uri("/healthz")), RouteClass::Other);
}

#[test]
fn a_hook_token_opens_only_its_own_plugins_hook_route() {
    let token = hook("acme");
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Hook(Some(plugin("acme")))),
        Ok(LoopbackCredential::Hook(plugin("acme")))
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Hook(Some(plugin("other")))),
        Err(Rejection::ScopeMismatch),
        "a hook token minted for one plugin does not open another plugin's hooks"
    );
    for route in [RouteClass::Inference, RouteClass::Mcp] {
        assert_eq!(
            authenticate(&token, &secret(), &route),
            Err(Rejection::SecretMismatch),
            "a leaked hooks.json drives no inference or MCP traffic: {route:?}"
        );
    }
    for route in [RouteClass::Otel, RouteClass::Other] {
        assert_eq!(
            authenticate(&token, &secret(), &route),
            Err(Rejection::SecretMismatch),
            "{route:?}"
        );
    }
}

#[test]
fn a_host_token_opens_inference_and_mcp_but_neither_hooks_nor_otel() {
    let token = host("claude-desktop");
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Inference),
        Ok(LoopbackCredential::Host(HostId::new("claude-desktop")))
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Mcp),
        Ok(LoopbackCredential::Host(HostId::new("claude-desktop")))
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Hook(Some(plugin("acme")))),
        Err(Rejection::ScopeMismatch),
        "a host token is not a hook credential"
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Otel),
        Err(Rejection::ScopeMismatch),
        "OTLP ingest is forwarded only for the raw secret"
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Other),
        Err(Rejection::ScopeMismatch)
    );
}

#[test]
fn a_token_for_an_unknown_host_is_no_credential_at_all() {
    let token = host("not-a-known-host");
    for route in [
        RouteClass::Inference,
        RouteClass::Mcp,
        RouteClass::Otel,
        RouteClass::Other,
    ] {
        assert_eq!(
            authenticate(&token, &secret(), &route),
            Err(Rejection::SecretMismatch),
            "{route:?}"
        );
    }
}

#[test]
fn the_raw_secret_opens_every_route_except_a_hook_route() {
    for route in [
        RouteClass::Inference,
        RouteClass::Mcp,
        RouteClass::Otel,
        RouteClass::Other,
    ] {
        assert_eq!(
            authenticate(SECRET, &secret(), &route),
            Ok(LoopbackCredential::Secret),
            "{route:?}"
        );
    }
    assert_eq!(
        authenticate(SECRET, &secret(), &RouteClass::Hook(Some(plugin("acme")))),
        Err(Rejection::ScopeMismatch),
        "hooks carry only the per-plugin token; the secret never appears on that path"
    );
    assert_eq!(
        authenticate(SECRET, &secret(), &RouteClass::Hook(None)),
        Err(Rejection::ScopeMismatch),
        "a hook route with no plugin id has no scope to match"
    );
}

#[test]
fn an_empty_or_wrong_credential_is_refused_on_every_route() {
    for route in [
        RouteClass::Hook(Some(plugin("acme"))),
        RouteClass::Inference,
        RouteClass::Mcp,
        RouteClass::Otel,
        RouteClass::Other,
    ] {
        assert_eq!(
            authenticate("", &secret(), &route),
            Err(Rejection::NoCredential),
            "{route:?}"
        );
    }
    for route in [RouteClass::Inference, RouteClass::Mcp, RouteClass::Otel] {
        assert_eq!(
            authenticate("wrong-secret", &secret(), &route),
            Err(Rejection::SecretMismatch),
            "{route:?}"
        );
    }
}

#[test]
fn rejections_map_to_403_for_a_missing_or_wrong_secret_and_401_for_a_wrong_scope() {
    assert_eq!(
        Rejection::NoCredential.status(),
        http::StatusCode::FORBIDDEN
    );
    assert_eq!(
        Rejection::SecretMismatch.status(),
        http::StatusCode::FORBIDDEN
    );
    assert_eq!(
        Rejection::ScopeMismatch.status(),
        http::StatusCode::UNAUTHORIZED
    );
    assert_eq!(Rejection::NoCredential.reason(), "no-credential");
    assert_eq!(Rejection::ScopeMismatch.reason(), "scope-mismatch");
}

#[test]
fn derived_tokens_are_stable_per_scope_and_distinct_across_scopes_and_secrets() {
    assert_eq!(hook("acme"), hook("acme"));
    assert_ne!(hook("acme"), hook("other"));
    assert_ne!(hook("claude-desktop"), host("claude-desktop"));
    assert_ne!(
        host("claude-desktop"),
        host_token(
            &LoopbackSecret::new("another-secret"),
            &HostId::new("claude-desktop")
        )
        .as_str(),
        "rotating the secret invalidates every derived token"
    );
    assert!(
        !hook("acme").contains(SECRET),
        "a derived token never carries the secret"
    );
}
