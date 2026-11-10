//! Signing-key rotation with an overlap window.
//!
//! After a rotation the replaced key stays a verify-only member of the set
//! until its expiry: tokens it signed keep validating through the full RS256
//! path, the JWKS keeps publishing it, and from the expiry on both stop. The
//! verify-only key set serialises without private material.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier, RegisteredClaims, TokenType,
    UserType,
};
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};
use systemprompt_security::keys::{
    RetiredJwk, RsaSigningKey, SigningKeys, TokenVerificationKeys, VerificationKeyring,
    retired_jwk, rotation_from,
};

const ISSUER: &str = "https://issuer.test";
const SEED: [u8; 32] = [7; 32];

fn claims() -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        registered: RegisteredClaims {
            sub: "user-1".to_owned(),
            iat: now.timestamp(),
            exp: (now + Duration::hours(1)).timestamp(),
            nbf: Some(now.timestamp()),
            iss: ISSUER.to_owned(),
            aud: vec![JwtAudience::Api],
            jti: "jti-rotation".to_owned(),
        },
        custom: CustomClaims {
            scope: vec![Permission::User],
            username: "u".to_owned(),
            email: "u@example.com".to_owned(),
            user_type: UserType::User,
            roles: vec!["user".to_owned()],
            attributes: std::collections::BTreeMap::new(),
            client_id: Some(ClientId::new("c")),
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(SessionId::new("s")),
            rate_limit_tier: Some(RateLimitTier::User),
            plugin_id: None,
            act: None,
            purpose: None,
            interaction: None,
            browser_session: false,
        },
    }
}

fn mint(key: &RsaSigningKey) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid().to_owned());
    let der = key.private_key().to_pkcs1_der().expect("der");
    encode(
        &header,
        &claims(),
        &EncodingKey::from_rsa_der(der.as_bytes()),
    )
    .expect("mint")
}

fn verify(keys: &SigningKeys, token: &str) -> Result<JwtClaims, AuthError> {
    decode_rs256_claims(keys, token, &ValidationPolicy::session_context(ISSUER))
}

fn rotated(retired_until: DateTime<Utc>) -> (RsaSigningKey, SigningKeys) {
    let old = systemprompt_test_fixtures::next_test_key();
    let (next, rotation) = rotation_from(&old, retired_until).expect("rotation");
    let retired =
        retired_jwk(&rotation.previous.public_pem, rotation.previous.expires_at).expect("retired");
    let keys = SigningKeys::with_retired(next, retired, &SEED).expect("keys");
    (old, keys)
}

#[test]
fn a_token_signed_by_the_retired_key_verifies_during_the_overlap() {
    let (old, keys) = rotated(Utc::now() + Duration::hours(1));
    let claims = verify(&keys, &mint(&old)).expect("retired-key token verifies");
    assert_eq!(claims.registered.jti, "jti-rotation");
    assert_ne!(keys.active_kid(), old.kid());
}

#[test]
fn a_token_signed_by_the_new_active_key_verifies() {
    let (_, keys) = rotated(Utc::now() + Duration::hours(1));
    let claims = verify(&keys, &mint(keys.signing_key())).expect("active-key token verifies");
    assert_eq!(claims.registered.sub, "user-1");
}

#[test]
fn a_token_signed_by_the_retired_key_fails_after_its_expiry() {
    let (old, keys) = rotated(Utc::now() - Duration::seconds(1));
    let err = verify(&keys, &mint(&old)).expect_err("expired retired key accepted");
    assert!(
        matches!(&err, AuthError::UnknownKid(kid) if kid == old.kid()),
        "{err}"
    );
}

#[test]
fn without_a_retired_key_the_replaced_key_is_unknown() {
    let old = systemprompt_test_fixtures::next_test_key();
    let (next, _) = rotation_from(&old, Utc::now() + Duration::hours(1)).expect("rotation");
    let keys = SigningKeys::new(next, &SEED).expect("keys");
    let err = verify(&keys, &mint(&old)).expect_err("replaced key accepted");
    assert!(matches!(err, AuthError::UnknownKid(_)), "{err}");
}

#[test]
fn the_jwks_publishes_the_retired_key_only_until_it_expires() {
    let (old, keys) = rotated(Utc::now() + Duration::hours(1));
    let kids: Vec<String> = keys.jwks().keys.into_iter().map(|k| k.kid).collect();
    assert_eq!(
        kids,
        vec![keys.active_kid().to_owned(), old.kid().to_owned()]
    );

    let (_, expired) = rotated(Utc::now() - Duration::seconds(1));
    let kids: Vec<String> = expired.jwks().keys.into_iter().map(|k| k.kid).collect();
    assert_eq!(kids, vec![expired.active_kid().to_owned()]);
}

#[test]
fn the_rotation_retires_the_public_half_of_the_current_key() {
    let old = systemprompt_test_fixtures::next_test_key();
    let until = Utc::now() + Duration::hours(2);
    let (next, rotation) = rotation_from(&old, until).expect("rotation");
    assert!(rotation.previous.public_pem.contains("BEGIN PUBLIC KEY"));
    assert!(!rotation.previous.public_pem.contains("PRIVATE"));
    assert_eq!(rotation.previous.expires_at, until);
    let reloaded = RsaSigningKey::from_pkcs8_pem(&rotation.active_private_pem).expect("pem");
    assert_eq!(reloaded.kid(), next.kid());
    let retired = retired_jwk(&rotation.previous.public_pem, until).expect("retired");
    assert_eq!(retired.jwk.kid, old.kid());
}

#[test]
fn the_verification_key_set_round_trips_without_private_material() {
    let (old, keys) = rotated(Utc::now() + Duration::hours(1));
    let json = serde_json::to_string(keys.verification_keys()).expect("encode");
    assert!(
        !json.contains("\"d\""),
        "private exponent serialised: {json}"
    );
    let parsed: TokenVerificationKeys = serde_json::from_str(&json).expect("decode");
    assert_eq!(&parsed, keys.verification_keys());

    let ring = VerificationKeyring::new(parsed).expect("ring");
    assert!(ring.decoding_key_for_kid(old.kid()).is_some());
    assert!(ring.decoding_key_for_kid(keys.active_kid()).is_some());
    assert!(ring.decoding_key_for_kid("unknown").is_none());
}

#[test]
fn the_keyring_honours_the_retired_expiry_at_the_given_instant() {
    let old = systemprompt_test_fixtures::next_test_key();
    let active = systemprompt_test_fixtures::next_test_key();
    let until = Utc::now() + Duration::minutes(5);
    let ring = VerificationKeyring::new(TokenVerificationKeys {
        active: active.jwk(),
        retired: Some(RetiredJwk {
            jwk: old.jwk(),
            expires_at: until,
        }),
    })
    .expect("ring");
    assert!(
        ring.decoding_key_for_kid_at(old.kid(), until - Duration::seconds(1))
            .is_some()
    );
    assert!(ring.decoding_key_for_kid_at(old.kid(), until).is_none());
    assert!(
        ring.decoding_key_for_kid_at(active.kid(), until + Duration::days(1))
            .is_some()
    );
}
