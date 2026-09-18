//! The guard for URLs the GUI hands to the OS browser: absolute `https://`
//! anywhere, plain `http://` only to the machine itself. A self-hosted gateway
//! is reached over `http://localhost:8080`, so refusing it broke the Connect
//! button and every "open in browser" the console offers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_bridge::wire::external_url::{ExternalUrl, ExternalUrlRejected};

#[test]
fn loopback_http_is_allowed_for_a_local_gateway() {
    for target in [
        "http://localhost:8080/admin/connectors?expected_user=48a88a36",
        "http://127.0.0.1:8080/admin/connectors",
        "http://[::1]:3000/",
        "http://LOCALHOST:8080/x",
    ] {
        let url = ExternalUrl::parse(target).expect(target);
        assert_eq!(url.as_str(), target);
    }
}

#[test]
fn remote_http_is_refused() {
    for target in [
        "http://astounddigital.atlassian.net/",
        "http://169.254.169.254/latest/meta-data/",
        "http://example.com.localhost.evil.com/",
    ] {
        assert!(
            matches!(
                ExternalUrl::parse(target),
                Err(ExternalUrlRejected::NotHttps(_))
            ),
            "{target} should be refused"
        );
    }
}

#[test]
fn https_anywhere_is_allowed_and_other_schemes_are_refused() {
    assert!(ExternalUrl::parse("https://astounddigital.atlassian.net/").is_ok());
    for target in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "sp://settings",
        "ftp://host/x",
    ] {
        assert!(
            ExternalUrl::parse(target).is_err(),
            "{target} should be refused"
        );
    }
}

#[test]
fn control_characters_are_refused_before_scheme() {
    assert!(matches!(
        ExternalUrl::parse("https://example.com/\u{0}"),
        Err(ExternalUrlRejected::ControlCharacters)
    ));
}
