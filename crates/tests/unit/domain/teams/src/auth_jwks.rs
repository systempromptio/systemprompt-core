//! Wiremock coverage for the network-bound JWKS path of
//! `ActivityTokenVerifier`.
//!
//! `with_openid_url` (the `test` seam) redirects `OpenID` metadata
//! discovery to a loopback mock that serves the config document and a JWKS
//! built from the committed test keypair. Tokens are minted in-process with the
//! matching private key, so the signature, claim, and key-lookup checks run
//! end-to-end without touching the live Bot Connector.

use base64::Engine;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::traits::PublicKeyParts;
use serde::Serialize;
use systemprompt_teams::TeamsError;
use systemprompt_teams::auth::ActivityTokenVerifier;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PRIVATE_PEM: &str = include_str!("../keys/test_priv.pem");
const PUBLIC_PEM: &str = include_str!("../keys/test_pub.pem");

const ISSUER: &str = "https://api.botframework.com";
const AUDIENCE: &str = "app-1";
const SERVICE_URL: &str = "https://smba.trafficmanager.net/uk/";
const KID: &str = "test-kid";

#[derive(Serialize)]
struct Claims {
    iss: String,
    aud: String,
    exp: u64,
    serviceurl: String,
}

fn mint(kid: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let claims = Claims {
        iss: ISSUER.to_owned(),
        aud: AUDIENCE.to_owned(),
        exp: now + 3600,
        serviceurl: SERVICE_URL.to_owned(),
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    let key = EncodingKey::from_rsa_pem(PRIVATE_PEM.as_bytes()).expect("encode key");
    encode(&header, &claims, &key).expect("mint token")
}

// The committed public key as a JWK, base64url-encoding the modulus/exponent
// the way the Bot Connector's published key set does.
fn jwk(kid: &str) -> serde_json::Value {
    let key = RsaPublicKey::from_public_key_pem(PUBLIC_PEM).expect("parse public pem");
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.n().to_bytes_be());
    let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.e().to_bytes_be());
    serde_json::json!({ "kty": "RSA", "use": "sig", "alg": "RS256", "kid": kid, "n": n, "e": e })
}

async fn mount_openid(server: &MockServer, jwk_kid: &str) {
    Mock::given(method("GET"))
        .and(path("/openid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jwks_uri": format!("{}/jwks", server.uri()),
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "keys": [jwk(jwk_kid)] })),
        )
        .mount(server)
        .await;
}

fn verifier(server: &MockServer) -> ActivityTokenVerifier {
    ActivityTokenVerifier::with_openid_url(
        reqwest::Client::new(),
        AUDIENCE,
        format!("{}/openid", server.uri()),
    )
}

async fn jwks_hits(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .expect("requests recorded")
        .into_iter()
        .filter(|r| r.url.path() == "/jwks")
        .count()
}

#[tokio::test]
async fn verifies_a_token_against_the_fetched_jwks() {
    let server = MockServer::start().await;
    mount_openid(&server, KID).await;

    let claims = verifier(&server)
        .verify(&mint(KID), SERVICE_URL, 0)
        .await
        .expect("a well-formed token validates against the served key");
    assert_eq!(claims.serviceurl.as_deref(), Some(SERVICE_URL));
}

#[tokio::test]
async fn unknown_kid_triggers_a_fetch_and_then_fails_closed() {
    let server = MockServer::start().await;
    // The served key set carries only KID; a token signed under a different kid
    // forces a fetch (cache miss) and then a not-found rejection.
    mount_openid(&server, KID).await;

    let err = verifier(&server)
        .verify(&mint("rotated-kid"), SERVICE_URL, 0)
        .await
        .expect_err("a kid absent from the JWKS is rejected after refresh");
    assert!(
        matches!(err, TeamsError::TokenValidation(_)),
        "expected TokenValidation, got {err:?}"
    );

    assert_eq!(
        jwks_hits(&server).await,
        1,
        "the cache miss drove exactly one JWKS fetch"
    );
}

#[tokio::test]
async fn the_production_constructor_rejects_a_malformed_token_without_fetching() {
    // `ActivityTokenVerifier::new` wires the hardcoded Bot Connector `OpenID`
    // endpoint. A token whose header cannot even be decoded fails in
    // `decode_header` before any key fetch, so this covers the production
    // constructor without a live metadata call.
    let verifier = ActivityTokenVerifier::new(reqwest::Client::new(), AUDIENCE);
    let err = verifier
        .verify("not-a-jwt", SERVICE_URL, 0)
        .await
        .expect_err("a malformed token is rejected before the JWKS is fetched");
    assert!(
        matches!(err, TeamsError::TokenValidation(_)),
        "expected TokenValidation, got {err:?}"
    );
}

#[tokio::test]
async fn a_jwk_with_unparseable_key_material_is_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/openid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jwks_uri": format!("{}/jwks", server.uri()),
        })))
        .mount(&server)
        .await;
    // The kid matches, so key lookup succeeds, but the modulus is not valid
    // base64url — building the decoding key fails.
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "keys": [{ "kty": "RSA", "use": "sig", "alg": "RS256", "kid": KID, "n": "!!!", "e": "AQAB" }],
        })))
        .mount(&server)
        .await;

    let err = verifier(&server)
        .verify(&mint(KID), SERVICE_URL, 0)
        .await
        .expect_err("a JWK whose modulus cannot decode yields no usable key");
    assert!(
        matches!(err, TeamsError::TokenValidation(_)),
        "expected TokenValidation, got {err:?}"
    );
}

#[tokio::test]
async fn a_second_verify_is_served_from_the_key_cache() {
    let server = MockServer::start().await;
    mount_openid(&server, KID).await;
    let verifier = verifier(&server);

    verifier
        .verify(&mint(KID), SERVICE_URL, 0)
        .await
        .expect("first verify fetches the key set");
    verifier
        .verify(&mint(KID), SERVICE_URL, 10)
        .await
        .expect("second verify within the TTL hits the in-process cache");

    assert_eq!(
        jwks_hits(&server).await,
        1,
        "a cached key set is reused without a second JWKS fetch"
    );
}

#[tokio::test]
async fn an_expired_key_cache_is_refetched() {
    let server = MockServer::start().await;
    mount_openid(&server, KID).await;
    let verifier = verifier(&server);

    verifier
        .verify(&mint(KID), SERVICE_URL, 0)
        .await
        .expect("first verify populates the cache at t=0");
    // The JWKS cache TTL is 24h; advancing `now_unix` past it forces a refresh.
    verifier
        .verify(&mint(KID), SERVICE_URL, 24 * 60 * 60 + 1)
        .await
        .expect("a verify past the TTL refetches the key set");

    assert_eq!(
        jwks_hits(&server).await,
        2,
        "an expired key cache triggers a fresh JWKS fetch"
    );
}
