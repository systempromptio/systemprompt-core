//! Redirect URI policy at registration time (RFC 7591 / RFC 8252).

use systemprompt_oauth::services::validation::{
    validate_client_metadata_uri, validate_registration_redirect_uris,
};

fn uris(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn web_client_accepts_https() {
    validate_registration_redirect_uris("web", &uris(&["https://app.example/cb"]))
        .expect("https is the ordinary web redirect");
}

#[test]
fn web_client_accepts_loopback_http_on_any_port() {
    for uri in [
        "http://127.0.0.1/cb",
        "http://127.0.0.1:53281/callback",
        "http://localhost:3000/cb",
        "http://[::1]:8080/cb",
    ] {
        validate_registration_redirect_uris("web", &uris(&[uri]))
            .unwrap_or_else(|e| panic!("{uri} is a loopback redirect: {e}"));
    }
}

#[test]
fn web_client_refuses_plain_http_to_a_remote_host() {
    let err = validate_registration_redirect_uris("web", &uris(&["http://attacker.example/cb"]))
        .unwrap_err();
    assert!(err.to_string().contains("https"), "{err}");
}

#[test]
fn every_client_refuses_script_schemes_and_fragments() {
    for app in ["web", "native"] {
        for uri in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "https://app.example/cb#fragment",
            "not a url",
        ] {
            assert!(
                validate_registration_redirect_uris(app, &uris(&[uri])).is_err(),
                "{app} must refuse {uri}"
            );
        }
    }
}

#[test]
fn one_bad_uri_fails_the_whole_registration() {
    let err = validate_registration_redirect_uris(
        "web",
        &uris(&["https://app.example/cb", "http://attacker.example/cb"]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("attacker.example"), "{err}");
}

#[test]
fn native_client_accepts_private_use_scheme() {
    validate_registration_redirect_uris(
        "native",
        &uris(&["com.example.app:/oauth/callback", "cursor://anysphere/cb"]),
    )
    .expect("private-use schemes are the RFC 8252 native redirect");
}

#[test]
fn web_client_refuses_private_use_scheme() {
    let err =
        validate_registration_redirect_uris("web", &uris(&["com.example.app:/oauth/callback"]))
            .unwrap_err();
    assert!(err.to_string().contains("native"), "{err}");
}

#[test]
fn client_metadata_uri_must_be_http_or_https() {
    validate_client_metadata_uri("client_uri", None).expect("absent is fine");
    validate_client_metadata_uri("client_uri", Some("https://vendor.example"))
        .expect("https is fine");
    let err = validate_client_metadata_uri("logo_uri", Some("javascript:alert(1)")).unwrap_err();
    assert!(err.to_string().contains("logo_uri"), "{err}");
}
