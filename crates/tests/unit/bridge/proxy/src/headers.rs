//! `x-session-id` belongs to the client: the proxy forwards a client value
//! unchanged and never synthesizes one, not even from configured extras.

use std::collections::BTreeMap;

use http::{HeaderMap, HeaderValue};
use systemprompt_bridge::proxy::forward::headers::{UpstreamHeaderInputs, build_upstream_headers};
use systemprompt_identifiers::headers::SESSION_ID;

fn build(src: &HeaderMap, extra: &BTreeMap<String, String>) -> HeaderMap {
    build_upstream_headers(&UpstreamHeaderInputs {
        src,
        bearer: "jwt",
        gateway_conversation_id: None,
        extra,
        attest: None,
    })
    .unwrap()
}

fn session_values(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(SESSION_ID)
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_client_session_id_is_forwarded_unchanged() {
    let mut src = HeaderMap::new();
    src.insert(SESSION_ID, HeaderValue::from_static("ses_OpenCode42"));
    let built = build(&src, &BTreeMap::new());
    assert_eq!(session_values(&built), vec!["ses_OpenCode42".to_owned()]);
}

#[test]
fn no_session_id_is_synthesized_when_the_client_sent_none() {
    let built = build(&HeaderMap::new(), &BTreeMap::new());
    assert!(session_values(&built).is_empty());
    assert_eq!(
        built.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer jwt")
    );
}

#[test]
fn configured_extra_headers_cannot_inject_or_rewrite_it() {
    let extra = BTreeMap::from([
        ("X-Session-Id".to_owned(), "bridge-made".to_owned()),
        ("x-tenant".to_owned(), "t1".to_owned()),
    ]);
    assert!(session_values(&build(&HeaderMap::new(), &extra)).is_empty());

    let mut src = HeaderMap::new();
    src.insert(SESSION_ID, HeaderValue::from_static("ses_client"));
    let built = build(&src, &extra);
    assert_eq!(session_values(&built), vec!["ses_client".to_owned()]);
    assert_eq!(
        built.get("x-tenant").and_then(|v| v.to_str().ok()),
        Some("t1")
    );
}
