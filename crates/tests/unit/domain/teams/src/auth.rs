//! Tests for Bot Framework activity-token validation.
//!
//! Tokens are minted in-process with a fixed RSA keypair (below), so the
//! cryptographic and claim checks run with no network and no RNG. The
//! network-bound JWKS fetch in `ActivityTokenVerifier` is exercised by the
//! integration harness, not here.

use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, encode};
use serde::Serialize;
use systemprompt_teams::TeamsError;
use systemprompt_teams::auth::validate_token;

const PRIVATE_PEM: &str = include_str!("../keys/test_priv.pem");
const PUBLIC_PEM: &str = include_str!("../keys/test_pub.pem");

const ISSUER: &str = "https://api.botframework.com";
const AUDIENCE: &str = "app-1";
const SERVICE_URL: &str = "https://smba.trafficmanager.net/uk/";

#[derive(Serialize)]
struct Claims {
    iss: String,
    aud: String,
    exp: u64,
    serviceurl: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn decoding_key() -> DecodingKey {
    DecodingKey::from_rsa_pem(PUBLIC_PEM.as_bytes()).unwrap()
}

fn mint(claims: &Claims) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-kid".to_owned());
    let key = EncodingKey::from_rsa_pem(PRIVATE_PEM.as_bytes()).unwrap();
    encode(&header, claims, &key).unwrap()
}

fn valid_claims() -> Claims {
    Claims {
        iss: ISSUER.to_owned(),
        aud: AUDIENCE.to_owned(),
        exp: now() + 3600,
        serviceurl: Some(SERVICE_URL.to_owned()),
    }
}

#[test]
fn accepts_a_well_formed_token() {
    let token = mint(&valid_claims());
    let claims = validate_token(&token, &decoding_key(), AUDIENCE, SERVICE_URL).unwrap();
    assert_eq!(claims.serviceurl.as_deref(), Some(SERVICE_URL));
}

#[test]
fn rejects_a_wrong_issuer() {
    let token = mint(&Claims {
        iss: "https://evil.example".to_owned(),
        ..valid_claims()
    });
    assert!(matches!(
        validate_token(&token, &decoding_key(), AUDIENCE, SERVICE_URL),
        Err(TeamsError::IssuerMismatch(_))
    ));
}

#[test]
fn rejects_a_wrong_audience() {
    let token = mint(&Claims {
        aud: "some-other-app".to_owned(),
        ..valid_claims()
    });
    assert!(matches!(
        validate_token(&token, &decoding_key(), AUDIENCE, SERVICE_URL),
        Err(TeamsError::AudienceMismatch(_))
    ));
}

#[test]
fn rejects_an_expired_token() {
    let token = mint(&Claims {
        exp: now() - 1000,
        ..valid_claims()
    });
    assert!(matches!(
        validate_token(&token, &decoding_key(), AUDIENCE, SERVICE_URL),
        Err(TeamsError::StaleToken)
    ));
}

#[test]
fn rejects_a_service_url_mismatch() {
    let token = mint(&Claims {
        serviceurl: Some("https://smba.elsewhere/".to_owned()),
        ..valid_claims()
    });
    assert!(matches!(
        validate_token(&token, &decoding_key(), AUDIENCE, SERVICE_URL),
        Err(TeamsError::TokenValidation(_))
    ));
}

#[test]
fn rejects_a_token_missing_the_serviceurl_claim() {
    let token = mint(&Claims {
        serviceurl: None,
        ..valid_claims()
    });
    assert!(matches!(
        validate_token(&token, &decoding_key(), AUDIENCE, SERVICE_URL),
        Err(TeamsError::TokenValidation(_))
    ));
}

#[test]
fn rejects_a_signature_from_the_wrong_key() {
    // A garbage token that is not signed by our key at all.
    let bogus = "eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3Qta2lkIn0.eyJpc3MiOiJ4In0.AAAA";
    assert!(validate_token(bogus, &decoding_key(), AUDIENCE, SERVICE_URL).is_err());
}
