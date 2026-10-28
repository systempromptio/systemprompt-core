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

#[test]
fn a_same_origin_or_headerless_request_passes() {
    assert!(same_origin(&HeaderMap::new()));
    assert!(same_origin(&headers(&[("sec-fetch-site", "same-origin")])));
    assert!(same_origin(&headers(&[("sec-fetch-site", "none")])));
    assert!(same_origin(&headers(&[
        ("host", "docs.example.test"),
        ("origin", "https://docs.example.test"),
    ])));
}

#[test]
fn a_cross_site_request_or_foreign_origin_fails() {
    assert!(!same_origin(&headers(&[("sec-fetch-site", "cross-site")])));
    assert!(!same_origin(&headers(&[("sec-fetch-site", "same-site")])));
    assert!(!same_origin(&headers(&[
        ("host", "docs.example.test"),
        ("origin", "https://evil.example.test"),
    ])));
}
