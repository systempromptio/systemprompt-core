//! Route-class credential matrix for the loopback proxy: the raw secret, a
//! per-plugin hook token and a per-host token each open exactly the routes
//! their surface is read from, and nothing else.

use http::Uri;
use systemprompt_bridge::ids::{LoopbackSecret, PluginId, ProxySecret};
use systemprompt_bridge::proxy::credential::{
    LoopbackCredential, Rejection, RouteClass, authenticate, classify_route, credentialed_hosts,
};
use systemprompt_bridge::proxy::forward::path::ProxyPath;
use systemprompt_bridge::proxy::scoped_token::{
    HostOwnsConfig, TokenScope, hook_token, host_token, verify,
};
use systemprompt_models::bridge::host::{ConfigOwner, HostKind};

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

fn host(id: HostKind) -> String {
    host_token(&LoopbackSecret::new(SECRET), id)
        .expect("the bridge owns this host's configuration")
        .as_str()
        .to_owned()
}

fn proxy_path(s: &str) -> ProxyPath {
    let uri: Uri = s.parse().expect("uri");
    ProxyPath::parse(&uri).expect("canonical path")
}

#[test]
fn routes_are_classified_by_the_surface_their_credential_is_read_from() {
    assert_eq!(
        classify_route(&proxy_path("/api/public/hooks/govern?plugin_id=acme")),
        RouteClass::Hook(Some(plugin("acme")))
    );
    assert_eq!(
        classify_route(&proxy_path("/api/public/hooks/track")),
        RouteClass::Hook(None)
    );
    assert_eq!(
        classify_route(&proxy_path("/v1/messages")),
        RouteClass::Inference
    );
    assert_eq!(
        classify_route(&proxy_path("/v1/chat/completions")),
        RouteClass::Inference
    );
    assert_eq!(classify_route(&proxy_path("/mcp/x")), RouteClass::Mcp);
    assert_eq!(classify_route(&proxy_path("/mcp")), RouteClass::Mcp);
    assert_eq!(classify_route(&proxy_path("/otel")), RouteClass::Otel);
    assert_eq!(
        classify_route(&proxy_path("/otel/v1/traces")),
        RouteClass::Otel
    );
    assert_eq!(
        classify_route(&proxy_path("/v1/bridge/manifest")),
        RouteClass::Other
    );
    assert_eq!(classify_route(&proxy_path("/healthz")), RouteClass::Other);
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
fn a_host_token_opens_inference_mcp_and_otel_but_not_hooks_or_other_routes() {
    let token = host(HostKind::ClaudeDesktop);
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Inference),
        Ok(LoopbackCredential::Host(HostKind::ClaudeDesktop))
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Mcp),
        Ok(LoopbackCredential::Host(HostKind::ClaudeDesktop))
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Hook(Some(plugin("acme")))),
        Err(Rejection::ScopeMismatch),
        "a host token is not a hook credential"
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Otel),
        Ok(LoopbackCredential::Host(HostKind::ClaudeDesktop)),
        "OTLP ingest is authenticated by the enrolled host credential"
    );
    assert_eq!(
        authenticate(&token, &secret(), &RouteClass::Other),
        Err(Rejection::ScopeMismatch)
    );
}

#[test]
fn a_host_token_minted_under_another_secret_is_no_credential_at_all() {
    let token = host_token(&LoopbackSecret::new("another-secret"), HostKind::Hermes)
        .expect("the bridge owns this host's configuration")
        .as_str()
        .to_owned();
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
    assert_ne!(hook("claude-desktop"), host(HostKind::ClaudeDesktop));
    assert_ne!(host(HostKind::ClaudeDesktop), host(HostKind::CodexCli));
    assert_ne!(
        host(HostKind::ClaudeDesktop),
        host_token(
            &LoopbackSecret::new("another-secret"),
            HostKind::ClaudeDesktop
        )
        .expect("the bridge owns this host's configuration")
        .as_str(),
        "rotating the secret invalidates every derived token"
    );
    assert!(
        !hook("acme").contains(SECRET),
        "a derived token never carries the secret"
    );
}

#[test]
fn no_host_token_is_derived_for_a_host_that_owns_its_configuration() {
    let secret = LoopbackSecret::new(SECRET);
    for kind in HostKind::ALL {
        match kind.config_owner() {
            ConfigOwner::Bridge => assert!(host_token(&secret, kind).is_ok(), "{kind}"),
            ConfigOwner::Host => {
                assert_eq!(
                    host_token(&secret, kind),
                    Err(HostOwnsConfig(kind)),
                    "{kind}"
                );
            },
        }
    }
    assert_eq!(
        host_token(&secret, HostKind::Harness),
        Err(HostOwnsConfig(HostKind::Harness))
    );
    assert!(
        HostOwnsConfig(HostKind::Harness)
            .to_string()
            .contains("harness")
    );
}

#[test]
fn the_proxy_recognises_a_host_token_only_for_bridge_configured_hosts() {
    let recognised: Vec<HostKind> = credentialed_hosts().collect();
    let expected: Vec<HostKind> = HostKind::ALL
        .into_iter()
        .filter(|kind| kind.config_owner() == ConfigOwner::Bridge)
        .collect();
    assert_eq!(recognised, expected);
    assert!(
        !recognised.contains(&HostKind::Harness),
        "harness presents no loopback credential"
    );
    for kind in recognised {
        let token = host(kind);
        for route in [RouteClass::Inference, RouteClass::Mcp, RouteClass::Otel] {
            assert_eq!(
                authenticate(&token, &secret(), &route),
                Ok(LoopbackCredential::Host(kind)),
                "{kind} on {route:?}"
            );
        }
    }
}

#[test]
fn no_credential_authenticates_as_a_host_that_owns_its_configuration() {
    let harness = TokenScope::Host(HostKind::Harness);
    let mut presented: Vec<String> = credentialed_hosts().map(host).collect();
    presented.push(SECRET.to_owned());
    presented.push(hook("harness"));
    for token in &presented {
        assert!(
            !verify(token, &secret(), &harness),
            "no token verifies under a scope the bridge never issues"
        );
        for route in [
            RouteClass::Hook(Some(plugin("harness"))),
            RouteClass::Inference,
            RouteClass::Mcp,
            RouteClass::Otel,
            RouteClass::Other,
        ] {
            assert_ne!(
                authenticate(token, &secret(), &route),
                Ok(LoopbackCredential::Host(HostKind::Harness)),
                "{route:?}"
            );
        }
    }
}
