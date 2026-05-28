use http::HeaderMap;
use systemprompt_api::services::proxy::auth::OAuthChallengeBuilder;

fn headers_with_host(host: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("host", host.parse().expect("host parses"));
    h
}

#[test]
fn resource_metadata_url_uses_host_when_allowlisted_loopback_alias() {
    // RFC 9728 dual-self-identity: client dialled 127.0.0.1, configured
    // base is localhost. The 401 challenge's resource_metadata MUST echo
    // 127.0.0.1 so it agrees with the discovery body (also host-derived).
    let headers = headers_with_host("127.0.0.1:8080");
    let url = OAuthChallengeBuilder::resource_metadata_url(
        &headers,
        "http://localhost:8080",
        "/api/v1/mcp/sharepoint-sim/mcp",
    )
    .expect("ok");
    assert_eq!(
        url,
        "http://127.0.0.1:8080/.well-known/oauth-protected-resource/api/v1/mcp/sharepoint-sim/mcp"
    );
}

#[test]
fn resource_metadata_url_uses_host_when_matches_configured() {
    let headers = headers_with_host("gateway.example.com");
    let url = OAuthChallengeBuilder::resource_metadata_url(
        &headers,
        "https://gateway.example.com",
        "/api/v1/mcp/foo/mcp",
    )
    .expect("ok");
    assert_eq!(
        url,
        "https://gateway.example.com/.well-known/oauth-protected-resource/api/v1/mcp/foo/mcp"
    );
}

#[test]
fn resource_metadata_url_falls_back_when_host_not_allowlisted() {
    // Host-header injection defence: an attacker-controlled Host must not
    // rewrite the challenge URL away from the operator-configured base.
    let headers = headers_with_host("evil.com");
    let url = OAuthChallengeBuilder::resource_metadata_url(
        &headers,
        "https://gateway.example.com",
        "/api/v1/mcp/foo/mcp",
    )
    .expect("ok");
    assert_eq!(
        url,
        "https://gateway.example.com/.well-known/oauth-protected-resource/api/v1/mcp/foo/mcp"
    );
}

#[test]
fn resource_metadata_url_falls_back_when_no_host_header() {
    let headers = HeaderMap::new();
    let url = OAuthChallengeBuilder::resource_metadata_url(
        &headers,
        "http://localhost:8080",
        "/api/v1/mcp/foo/mcp",
    )
    .expect("ok");
    assert_eq!(
        url,
        "http://localhost:8080/.well-known/oauth-protected-resource/api/v1/mcp/foo/mcp"
    );
}

#[test]
fn resource_metadata_url_localhost_to_127_via_configured_127() {
    // Reverse direction: configured 127.0.0.1, client dialled via
    // localhost. Both are loopback aliases — the challenge URL must use
    // the host the client used.
    let headers = headers_with_host("localhost:8080");
    let url = OAuthChallengeBuilder::resource_metadata_url(
        &headers,
        "http://127.0.0.1:8080",
        "/api/v1/mcp/foo/mcp",
    )
    .expect("ok");
    assert_eq!(
        url,
        "http://localhost:8080/.well-known/oauth-protected-resource/api/v1/mcp/foo/mcp"
    );
}

#[test]
fn resource_metadata_url_loopback_alias_not_offered_when_configured_public() {
    // A public configured base must NOT silently accept a loopback Host —
    // an attacker on the same LAN could otherwise spoof a challenge URL
    // pointing at their own loopback.
    let headers = headers_with_host("127.0.0.1");
    let url = OAuthChallengeBuilder::resource_metadata_url(
        &headers,
        "https://gateway.example.com",
        "/api/v1/mcp/foo/mcp",
    )
    .expect("ok");
    assert_eq!(
        url,
        "https://gateway.example.com/.well-known/oauth-protected-resource/api/v1/mcp/foo/mcp"
    );
}

#[test]
fn resource_metadata_url_errors_on_invalid_configured_base() {
    let headers = HeaderMap::new();
    let err =
        OAuthChallengeBuilder::resource_metadata_url(&headers, "not a url", "/api/v1/mcp/foo/mcp")
            .unwrap_err();
    // url::ParseError stringifies to a human-readable message; just confirm
    // we surface a parse error rather than producing a URL.
    let _ = err;
}
