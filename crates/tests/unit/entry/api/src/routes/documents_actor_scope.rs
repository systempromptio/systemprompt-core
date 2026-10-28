//! The same-origin check a browser session must pass on the documents
//! routes.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::routes::documents::actor_scope::same_origin;

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(*name, HeaderValue::from_static(value));
    }
    map
}

fn site() -> Vec<String> {
    vec!["https://docs.example.test".to_owned()]
}

#[test]
fn a_same_origin_or_headerless_request_passes() {
    assert!(same_origin(&HeaderMap::new(), &site()));
    assert!(same_origin(
        &headers(&[("sec-fetch-site", "same-origin")]),
        &site()
    ));
    assert!(same_origin(
        &headers(&[("sec-fetch-site", "none")]),
        &site()
    ));
    assert!(same_origin(
        &headers(&[("origin", "https://docs.example.test")]),
        &site()
    ));
}

#[test]
fn the_configured_origin_wins_over_a_proxied_host() {
    let proxied = headers(&[
        ("host", "internal-app.flycast:8080"),
        ("origin", "https://docs.example.test"),
    ]);
    assert!(same_origin(&proxied, &site()), "a proxy's Host is ignored");
    let forged_host = headers(&[
        ("host", "evil.example.test"),
        ("origin", "https://evil.example.test"),
    ]);
    assert!(
        !same_origin(&forged_host, &site()),
        "a matching Host is not enough"
    );
}

#[test]
fn a_cross_site_request_or_foreign_origin_fails() {
    assert!(!same_origin(
        &headers(&[("sec-fetch-site", "cross-site")]),
        &site()
    ));
    assert!(!same_origin(
        &headers(&[("sec-fetch-site", "same-site")]),
        &site()
    ));
    assert!(!same_origin(
        &headers(&[("origin", "https://evil.example.test")]),
        &site()
    ));
}

#[test]
fn without_a_configured_origin_the_host_must_match() {
    let same = headers(&[
        ("host", "docs.example.test"),
        ("origin", "https://docs.example.test"),
    ]);
    assert!(same_origin(&same, &[]));
    let foreign = headers(&[
        ("host", "docs.example.test"),
        ("origin", "https://evil.example.test"),
    ]);
    assert!(!same_origin(&foreign, &[]));
}
