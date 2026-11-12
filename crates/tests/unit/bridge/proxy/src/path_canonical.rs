//! A loopback request path is parsed once into `ProxyPath`; the route class
//! and the upstream URL are both derived from that value. Any path the URL
//! parser would normalise differently from its raw form is refused, so a
//! class decided on one path can never forward to another.

use std::sync::Arc;

use http::Uri;
use systemprompt_bridge::mcp_registry::{McpRegistrySlot, empty_slot};
use systemprompt_bridge::proxy::credential::{RouteClass, classify_route};
use systemprompt_bridge::proxy::forward::path::{PathRejection, ProxyPath};
use systemprompt_bridge::proxy::forward::route::{RouteResolution, resolve_route};
use systemprompt_identifiers::ValidatedUrl;

fn gateway() -> ValidatedUrl {
    ValidatedUrl::try_new("https://gateway.example.test").expect("valid gateway url")
}

fn parse(raw: &str) -> Result<ProxyPath, PathRejection> {
    let uri: Uri = raw.parse().expect("hyper accepts this request target");
    ProxyPath::parse(&uri)
}

#[test]
fn hostile_paths_are_refused_before_classification() {
    for (raw, expected) in [
        ("/v1/x/../../api/v1/foo", PathRejection::DotSegment),
        ("/v1/./messages", PathRejection::DotSegment),
        ("/otel/../../api/v1/x", PathRejection::DotSegment),
        ("/v1/x/..", PathRejection::DotSegment),
        (
            "/v1/x/%2e%2e/%2e%2e/api/v1/foo",
            PathRejection::EncodedReserved,
        ),
        ("/v1/x/%2E%2E/api", PathRejection::EncodedReserved),
        ("/v1/x/.%2e/api", PathRejection::EncodedReserved),
        ("/v1/a%2fb", PathRejection::EncodedReserved),
        ("/v1/a%2Fb", PathRejection::EncodedReserved),
        ("/v1/a%5c..%5capi", PathRejection::EncodedReserved),
        ("/v1/a%00b", PathRejection::EncodedReserved),
        ("//evil.example/v1/messages", PathRejection::EmptySegment),
        ("/v1//messages", PathRejection::EmptySegment),
        ("*", PathRejection::NotOriginForm),
    ] {
        assert_eq!(parse(raw), Err(expected), "{raw}");
    }
}

#[test]
fn a_backslash_path_is_refused() {
    let uri: Uri = "/v1/x/..\\..\\api/v1/foo"
        .parse()
        .expect("hyper accepts a backslash");
    assert_eq!(ProxyPath::parse(&uri), Err(PathRejection::Backslash));
}

#[test]
fn an_absolute_form_target_keeps_only_its_path() {
    let parsed = parse("http://127.0.0.1:48217/v1/messages?beta=true").expect("canonical");
    assert_eq!(parsed.path(), "/v1/messages");
    assert_eq!(parsed.query(), Some("beta=true"));
}

#[test]
fn the_class_and_the_upstream_url_come_from_the_same_parsed_path() {
    let slot: Arc<McpRegistrySlot> = empty_slot();
    for (raw, class, upstream_path) in [
        (
            "/v1/messages?beta=true",
            RouteClass::Inference,
            "/v1/messages",
        ),
        (
            "/v1/bridge/manifest",
            RouteClass::Other,
            "/v1/bridge/manifest",
        ),
        ("/otel/v1/traces", RouteClass::Otel, "/v1/otel/v1/traces"),
        ("/api/v1/foo", RouteClass::Other, "/api/v1/foo"),
    ] {
        let parsed = parse(raw).expect("canonical");
        assert_eq!(classify_route(&parsed), class, "{raw}");
        let RouteResolution::Gateway(url) = resolve_route(&parsed, &gateway(), &slot) else {
            panic!("{raw} resolves to the gateway");
        };
        assert_eq!(url.host_str(), Some("gateway.example.test"), "{raw}");
        assert_eq!(url.path(), upstream_path, "{raw}");
        let reparsed = url::Url::parse(url.as_str()).expect("upstream url reparses");
        assert_eq!(reparsed.path(), upstream_path, "{raw} survives a reparse");
    }
}

#[test]
fn a_gateway_base_with_a_path_prefix_keeps_the_routed_path_under_it() {
    let base = ValidatedUrl::try_new("https://gateway.example.test/tenant/").expect("url");
    let parsed = parse("/v1/messages?x=1").expect("canonical");
    let url = parsed.gateway_url(&base).expect("upstream url");
    assert_eq!(
        url.as_str(),
        "https://gateway.example.test/tenant/v1/messages?x=1"
    );
}

#[test]
fn a_trailing_slash_and_the_root_are_canonical() {
    assert_eq!(parse("/mcp/").expect("canonical").path(), "/mcp/");
    assert_eq!(parse("/").expect("canonical").path(), "/");
}

#[test]
fn the_hook_plugin_id_is_read_from_the_parsed_query() {
    let parsed =
        parse("/api/public/hooks/track?a=1&plugin_id=acme&plugin_id=other").expect("canonical");
    assert!(matches!(
        classify_route(&parsed),
        RouteClass::Hook(Some(id)) if id.as_str() == "acme"
    ));
}
