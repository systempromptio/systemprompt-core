//! The client attestation the proxy stamps on inference traffic: a verified
//! host token names the host, the raw secret only vouches for the channel,
//! and nothing a caller sends under the attestation header survives.

use std::collections::BTreeMap;

use http::{HeaderMap, HeaderValue};
use systemprompt_bridge::ids::HostId;
use systemprompt_bridge::proxy::credential::LoopbackCredential;
use systemprompt_bridge::proxy::forward::headers::{
    UpstreamHeaderInputs, build_upstream_headers, copy_request_headers, stamp_attestation,
};
use systemprompt_identifiers::SessionId;
use systemprompt_identifiers::headers::{CLIENT_ATTESTATION, CLIENT_KIND};
use systemprompt_models::bridge::profile::KNOWN_HOSTS;
use systemprompt_models::wire::origin::ClientKind;

fn inbound(client: Option<&str>, attestation: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static("claude-cli/2.0"));
    if let Some(client) = client {
        headers.insert(CLIENT_KIND, HeaderValue::from_str(client).unwrap());
    }
    if let Some(attestation) = attestation {
        headers.insert(
            CLIENT_ATTESTATION,
            HeaderValue::from_str(attestation).unwrap(),
        );
    }
    headers
}

fn value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

#[test]
fn a_host_token_stamps_host_token_and_overwrites_the_declared_client() {
    let mut headers = HeaderMap::new();
    copy_request_headers(&inbound(Some("pi"), None), &mut headers);
    let credential = LoopbackCredential::Host(HostId::new("opencode"));
    stamp_attestation(&mut headers, Some(&credential)).unwrap();
    assert_eq!(value(&headers, CLIENT_ATTESTATION), Some("host-token"));
    assert_eq!(value(&headers, CLIENT_KIND), Some("opencode"));
    assert_eq!(value(&headers, "user-agent"), Some("claude-cli/2.0"));
}

#[test]
fn every_known_host_maps_onto_the_canonical_wire_vocabulary() {
    for host in KNOWN_HOSTS {
        let mut headers = HeaderMap::new();
        let credential = LoopbackCredential::Host(HostId::new(*host));
        stamp_attestation(&mut headers, Some(&credential)).unwrap();
        let expected = ClientKind::from_bridge_host_id(host).unwrap().as_str();
        assert_eq!(value(&headers, CLIENT_KIND), Some(expected), "{host}");
        assert_eq!(value(&headers, CLIENT_ATTESTATION), Some("host-token"));
    }
    let mut headers = HeaderMap::new();
    let codex = LoopbackCredential::Host(HostId::new("codex-cli"));
    stamp_attestation(&mut headers, Some(&codex)).unwrap();
    assert_eq!(
        value(&headers, CLIENT_KIND),
        Some("codex"),
        "the bridge host id is internal; the wire carries the ClientKind string"
    );
}

#[test]
fn the_secret_stamps_bridge_secret_and_passes_a_declaration_through() {
    let mut headers = HeaderMap::new();
    copy_request_headers(&inbound(Some("pi"), None), &mut headers);
    stamp_attestation(&mut headers, Some(&LoopbackCredential::Secret)).unwrap();
    assert_eq!(value(&headers, CLIENT_ATTESTATION), Some("bridge-secret"));
    assert_eq!(value(&headers, CLIENT_KIND), Some("pi"));

    let mut bare = HeaderMap::new();
    copy_request_headers(&inbound(None, None), &mut bare);
    stamp_attestation(&mut bare, Some(&LoopbackCredential::Secret)).unwrap();
    assert_eq!(value(&bare, CLIENT_ATTESTATION), Some("bridge-secret"));
    assert_eq!(value(&bare, CLIENT_KIND), None);
}

#[test]
fn an_inbound_attestation_header_is_always_removed() {
    let src = inbound(Some("opencode"), Some("host-token"));
    let mut copied = HeaderMap::new();
    copy_request_headers(&src, &mut copied);
    assert_eq!(
        value(&copied, CLIENT_ATTESTATION),
        None,
        "the strip is structural: hop-by-hop, before any stamping"
    );
    assert_eq!(value(&copied, CLIENT_KIND), Some("opencode"));

    let built = build_upstream_headers(&UpstreamHeaderInputs {
        src: &src,
        bearer: "jwt",
        session_id: &SessionId::new("session"),
        gateway_conversation_id: None,
        extra: &BTreeMap::new(),
        attest: Some(&LoopbackCredential::Secret),
    })
    .unwrap();
    assert_eq!(
        value(&built, CLIENT_ATTESTATION),
        Some("bridge-secret"),
        "the caller's claim of a host token is replaced by what the proxy verified"
    );
    assert_eq!(value(&built, "authorization"), Some("Bearer jwt"));
}

#[test]
fn hook_and_mcp_routes_stamp_nothing_and_strip_the_declaration() {
    let src = inbound(Some("pi"), Some("bridge-secret"));
    let built = build_upstream_headers(&UpstreamHeaderInputs {
        src: &src,
        bearer: "jwt",
        session_id: &SessionId::new("session"),
        gateway_conversation_id: None,
        extra: &BTreeMap::new(),
        attest: None,
    })
    .unwrap();
    assert_eq!(value(&built, CLIENT_ATTESTATION), None);
    assert_eq!(value(&built, CLIENT_KIND), None);

    let mut headers = HeaderMap::new();
    copy_request_headers(&src, &mut headers);
    let hook = LoopbackCredential::Hook(
        systemprompt_bridge::ids::PluginId::try_new("plugin").unwrap(),
    );
    stamp_attestation(&mut headers, Some(&hook)).unwrap();
    assert_eq!(value(&headers, CLIENT_ATTESTATION), None);
    assert_eq!(value(&headers, CLIENT_KIND), None);
}
