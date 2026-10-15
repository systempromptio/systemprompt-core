//! JWKS documents as real identity providers publish them (RFC 7517).
//!
//! Each fixture keeps the provider's member set — Entra ID's `x5t` / `x5c` /
//! `issuer` / `cloud_instance_name` with no `alg`, Google's `alg` + `use`,
//! Okta's and Auth0's extras, and sets mixing `EC` / `OKP` / encryption keys —
//! and asserts the set parses to exactly the keys an RS256 verifier may use.

use systemprompt_security::keys::{Jwk, Jwks, JwksClient};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn rsa_parts() -> (String, String) {
    let signing = systemprompt_test_fixtures::next_test_key();
    let jwk = Jwk::from_rsa_public_key(signing.public_key(), "x".to_owned());
    (jwk.n, jwk.e)
}

fn kids(jwks: &Jwks) -> Vec<&str> {
    jwks.keys.iter().map(|k| k.kid.as_str()).collect()
}

fn entra_jwks(n: &str, e: &str) -> serde_json::Value {
    serde_json::json!({
        "keys": [
            {
                "kty": "RSA",
                "use": "sig",
                "kid": "JYhAcTPMZ_LX6DBlOWQ7Hn0NeXE",
                "x5t": "JYhAcTPMZ_LX6DBlOWQ7Hn0NeXE",
                "n": n,
                "e": e,
                "x5c": ["MIIC/jCCAeagAwIBAgIJAOCJOVRxNKcNMA0GCSqGSIb3DQEBCwUA"],
                "issuer": "https://login.microsoftonline.com/{tenantid}/v2.0",
                "cloud_instance_name": "microsoftonline.com"
            },
            {
                "kty": "RSA",
                "use": "sig",
                "kid": "L1KfKFI_jnXbwWc22xZxw1sUHH0",
                "x5t": "L1KfKFI_jnXbwWc22xZxw1sUHH0",
                "n": n,
                "e": e,
                "x5c": ["MIIC/TCCAeWgAwIBAgIIUd7j/OIahkYwDQYJKoZIhvcNAQELBQAw"],
                "issuer": "https://login.microsoftonline.com/{tenantid}/v2.0"
            }
        ]
    })
}

#[test]
fn entra_jwks_without_alg_parses_every_key() {
    let (n, e) = rsa_parts();
    let jwks: Jwks = serde_json::from_value(entra_jwks(&n, &e)).expect("Entra JWKS parses");
    assert_eq!(
        kids(&jwks),
        ["JYhAcTPMZ_LX6DBlOWQ7Hn0NeXE", "L1KfKFI_jnXbwWc22xZxw1sUHH0"]
    );
    assert!(jwks.keys.iter().all(|k| k.alg == "RS256" && k.use_ == "sig"));
}

#[test]
fn google_jwks_parses() {
    let (n, e) = rsa_parts();
    let doc = serde_json::json!({
        "keys": [
            {"kty": "RSA", "alg": "RS256", "use": "sig", "kid": "g-1", "n": n, "e": e},
            {"e": e, "kty": "RSA", "kid": "g-2", "n": n, "alg": "RS256", "use": "sig"}
        ]
    });
    let jwks: Jwks = serde_json::from_value(doc).expect("Google JWKS parses");
    assert_eq!(kids(&jwks), ["g-1", "g-2"]);
}

#[test]
fn okta_and_auth0_jwks_parse() {
    let (n, e) = rsa_parts();
    let okta = serde_json::json!({
        "keys": [{"kty": "RSA", "alg": "RS256", "kid": "okta-1", "use": "sig", "e": e, "n": n}]
    });
    let auth0 = serde_json::json!({
        "keys": [{
            "kty": "RSA", "use": "sig", "n": n, "e": e, "kid": "auth0-1",
            "x5t": "abc", "x5c": ["MIIDDTCCAfWgAwIBAgIJ"], "alg": "RS256"
        }]
    });
    let okta: Jwks = serde_json::from_value(okta).expect("Okta JWKS parses");
    let auth0: Jwks = serde_json::from_value(auth0).expect("Auth0 JWKS parses");
    assert_eq!(kids(&okta), ["okta-1"]);
    assert_eq!(kids(&auth0), ["auth0-1"]);
}

#[test]
fn non_rsa_and_non_signing_keys_are_dropped_not_fatal() {
    let (n, e) = rsa_parts();
    let doc = serde_json::json!({
        "keys": [
            {"kty": "EC", "crv": "P-256", "kid": "ec-1", "use": "sig", "alg": "ES256",
             "x": "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU",
             "y": "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0"},
            {"kty": "OKP", "crv": "Ed25519", "kid": "okp-1",
             "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"},
            {"kty": "RSA", "use": "enc", "kid": "enc-1", "n": n, "e": e},
            {"kty": "RSA", "key_ops": ["encrypt"], "kid": "ops-enc", "n": n, "e": e},
            {"kty": "RSA", "alg": "RS512", "kid": "rs512", "n": n, "e": e},
            {"kty": "RSA", "use": "sig", "n": n, "e": e},
            {"kty": "RSA", "kid": "no-modulus", "e": e},
            {"kty": "RSA", "kid": "rsa-ok", "key_ops": ["verify"], "n": n, "e": e},
            "not even an object"
        ]
    });
    let jwks: Jwks = serde_json::from_value(doc).expect("mixed JWKS parses");
    assert_eq!(kids(&jwks), ["rsa-ok"]);
}

#[test]
fn a_document_without_keys_is_still_an_error() {
    assert!(serde_json::from_value::<Jwks>(serde_json::json!({"kids": []})).is_err());
}

#[test]
fn published_jwks_round_trips() {
    let signing = systemprompt_test_fixtures::next_test_key();
    let jwks = signing.jwks();
    let wire = serde_json::to_string(&jwks).expect("serialize");
    let parsed: Jwks = serde_json::from_str(&wire).expect("parse");
    assert_eq!(parsed, jwks);
}

#[tokio::test]
async fn jwks_client_resolves_a_kid_from_an_entra_shaped_document() {
    let server = MockServer::start().await;
    let (n, e) = rsa_parts();
    Mock::given(method("GET"))
        .and(path("/common/discovery/v2.0/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(entra_jwks(&n, &e)))
        .mount(&server)
        .await;
    let host = url::Url::parse(&server.uri())
        .expect("uri")
        .host_str()
        .expect("host")
        .to_owned();
    let client = JwksClient::new(vec![host]);
    let jwk = client
        .fetch_at(
            "https://login.microsoftonline.com/tenant/v2.0",
            &format!("{}/common/discovery/v2.0/keys", server.uri()),
            "L1KfKFI_jnXbwWc22xZxw1sUHH0",
        )
        .await
        .expect("Entra kid resolves");
    assert_eq!(jwk.n, n);
}
