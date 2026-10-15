//! Standards compliance of the Bot Connector activity-token path.
//!
//! The served JWKS mirrors the real Bot Framework document (`x5t`, `x5c`,
//! `endorsements`, no `alg`) with an `EC` key and an encryption key mixed in;
//! those must drop out without failing the set. A token without `aud` or with
//! a `crit` header is refused (RFC 7519 §4.1.3, RFC 7515 §4.1.11), and an
//! unknown `kid` refetches the set at most once per throttle window.

use base64::Engine;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::traits::PublicKeyParts;
use systemprompt_identifiers::TeamsAppId;
use systemprompt_teams::TeamsError;
use systemprompt_teams::auth::ActivityTokenVerifier;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PRIVATE_PEM: &str = include_str!("../keys/test_priv.pem");
const PUBLIC_PEM: &str = include_str!("../keys/test_pub.pem");
const AUDIENCE: &str = "app-1";
const SERVICE_URL: &str = "https://smba.trafficmanager.net/uk/";
const KID: &str = "bf-kid";

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn mint(kid: &str, claims: &serde_json::Value, crit: bool) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    if crit {
        header.crit = Some(vec!["exp".to_owned()]);
    }
    let key = EncodingKey::from_rsa_pem(PRIVATE_PEM.as_bytes()).expect("encode key");
    encode(&header, claims, &key).expect("mint token")
}

fn full_claims() -> serde_json::Value {
    serde_json::json!({
        "iss": "https://api.botframework.com",
        "aud": AUDIENCE,
        "exp": now() + 3600,
        "serviceurl": SERVICE_URL,
    })
}

async fn mount_bot_framework_jwks(server: &MockServer) {
    let key = RsaPublicKey::from_public_key_pem(PUBLIC_PEM).expect("parse public pem");
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.n().to_bytes_be());
    let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.e().to_bytes_be());
    Mock::given(method("GET"))
        .and(path("/openid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issuer": "https://api.botframework.com",
            "jwks_uri": format!("{}/jwks", server.uri()),
            "id_token_signing_alg_values_supported": ["RS256"],
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "keys": [
                {"kty": "EC", "crv": "P-256", "kid": "ec", "x": "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU",
                 "y": "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0"},
                {"kty": "RSA", "use": "enc", "kid": KID, "n": "AQAB", "e": "AQAB"},
                {"kty": "RSA", "use": "sig", "kid": KID, "x5t": KID, "n": n, "e": e,
                 "x5c": ["MIIDCzCCAfOgAwIBAgIQ"], "endorsements": ["msteams"]}
            ]
        })))
        .mount(server)
        .await;
}

fn verifier(server: &MockServer) -> ActivityTokenVerifier {
    ActivityTokenVerifier::with_openid_url(
        reqwest::Client::new(),
        TeamsAppId::new(AUDIENCE),
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
async fn a_bot_framework_jwks_with_foreign_keys_verifies() {
    let server = MockServer::start().await;
    mount_bot_framework_jwks(&server).await;
    verifier(&server)
        .verify(&mint(KID, &full_claims(), false), SERVICE_URL, 0)
        .await
        .expect("the RSA signing key is found despite EC and enc keys in the set");
}

#[tokio::test]
async fn a_token_without_aud_is_refused() {
    let server = MockServer::start().await;
    mount_bot_framework_jwks(&server).await;
    let mut claims = full_claims();
    claims.as_object_mut().expect("object").remove("aud");
    let err = verifier(&server)
        .verify(&mint(KID, &claims, false), SERVICE_URL, 0)
        .await
        .expect_err("aud is required");
    assert!(matches!(err, TeamsError::InvalidToken { .. }), "{err:?}");
}

#[tokio::test]
async fn a_crit_header_is_refused() {
    let server = MockServer::start().await;
    mount_bot_framework_jwks(&server).await;
    let err = verifier(&server)
        .verify(&mint(KID, &full_claims(), true), SERVICE_URL, 0)
        .await
        .expect_err("crit is not understood");
    assert!(matches!(err, TeamsError::InvalidToken { .. }), "{err:?}");
}

#[tokio::test]
async fn unknown_kid_refetches_at_most_once_per_window() {
    let server = MockServer::start().await;
    mount_bot_framework_jwks(&server).await;
    let verifier = verifier(&server);
    let token = mint("rotated", &full_claims(), false);
    for at in [0, 10, 59] {
        let err = verifier.verify(&token, SERVICE_URL, at).await;
        assert!(matches!(err, Err(TeamsError::UnknownSigningKey { .. })));
    }
    assert_eq!(
        jwks_hits(&server).await,
        1,
        "misses inside the window are throttled"
    );
    let _ = verifier.verify(&token, SERVICE_URL, 61).await;
    assert_eq!(
        jwks_hits(&server).await,
        2,
        "a miss after the window refetches"
    );
}
