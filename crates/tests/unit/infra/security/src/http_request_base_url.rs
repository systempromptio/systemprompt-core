use systemprompt_api::services::request_base_url::resolve;
use url::Url;

fn configured(url: &str) -> Url {
    Url::parse(url).expect("test configured url parses")
}

#[test]
fn missing_host_falls_back_to_configured() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(None, &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn empty_host_falls_back_to_configured() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some(""), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn whitespace_host_falls_back_to_configured() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("   "), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn host_matching_configured_is_accepted() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("gateway.example.com"), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn non_allowlisted_host_falls_back_to_configured() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("evil.com"), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn loopback_aliases_interchangeable_when_configured_is_loopback() {
    let cfg = configured("http://localhost:8080");

    let from_ip = resolve(Some("127.0.0.1:8080"), &cfg);
    assert_eq!(from_ip.as_str(), "http://127.0.0.1:8080");

    let from_name = resolve(Some("localhost:8080"), &cfg);
    assert_eq!(from_name.as_str(), "http://localhost:8080");
}

#[test]
fn loopback_alias_uses_http_scheme_even_if_configured_https() {
    // Loopback dev addresses don't speak TLS — force http.
    let cfg = configured("https://localhost:8080");
    let base = resolve(Some("127.0.0.1:8080"), &cfg);
    assert!(base.as_str().starts_with("http://"));
}

#[test]
fn non_loopback_public_host_does_not_get_loopback_alias() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("127.0.0.1"), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn host_with_path_is_rejected() {
    let cfg = configured("http://localhost:8080");
    let base = resolve(Some("localhost:8080/evil"), &cfg);
    assert_eq!(base.as_str(), "http://localhost:8080");
}

#[test]
fn resolved_origin_matches_advertised_base() {
    let cfg = configured("http://localhost:8080");
    let base = resolve(Some("127.0.0.1:8080"), &cfg);
    let expected = Url::parse("http://127.0.0.1:8080").expect("parse").origin();
    assert_eq!(base.origin(), &expected);
}

#[test]
fn a_host_on_a_port_the_gateway_does_not_serve_falls_back_to_configured() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("gateway.example.com:1"), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}

#[test]
fn a_host_naming_the_configured_default_port_explicitly_is_accepted() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("gateway.example.com:443"), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com:443");
}

#[test]
fn a_host_matching_a_configured_explicit_port_is_accepted_and_others_are_not() {
    let cfg = configured("https://gateway.example.com:8443");
    assert_eq!(
        resolve(Some("gateway.example.com:8443"), &cfg).as_str(),
        "https://gateway.example.com:8443"
    );
    assert_eq!(
        resolve(Some("gateway.example.com"), &cfg).as_str(),
        "https://gateway.example.com:8443"
    );
}

#[test]
fn a_host_with_a_non_numeric_port_falls_back_to_configured() {
    let cfg = configured("https://gateway.example.com");
    let base = resolve(Some("gateway.example.com:abc"), &cfg);
    assert_eq!(base.as_str(), "https://gateway.example.com");
}
