//! Signing-key rotation with an overlap window.
//!
//! After a rotation the replaced key stays a verify-only member of the set
//! until its expiry: tokens it signed keep validating through the full RS256
//! path, the JWKS keeps publishing it, and from the expiry on both stop. The
//! verify-only key set serialises without private material.

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_config::{PreviousSigningKey, SigningKeyState};
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier, RegisteredClaims, TokenType,
    UserType,
};
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};
use systemprompt_security::keys::{
    RetiredJwk, RotationMode, RsaSigningKey, SigningKeys, SigningKeysError, TokenVerificationKeys,
    VerificationKeyring, promote, retired_jwk, stage,
};

const ISSUER: &str = "https://issuer.test";
pub(crate) const SEED: [u8; 32] = [7; 32];

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

pub(crate) fn mint(key: &RsaSigningKey) -> String {
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

pub(crate) fn verify(keys: &SigningKeys, token: &str) -> Result<JwtClaims, AuthError> {
    verify_with(keys.keyring(), token)
}

pub(crate) fn verify_with(
    keyring: &VerificationKeyring,
    token: &str,
) -> Result<JwtClaims, AuthError> {
    decode_rs256_claims(keyring, token, &ValidationPolicy::session_context(ISSUER))
}

pub(crate) fn keys_after(next: RsaSigningKey, state: &SigningKeyState) -> SigningKeys {
    let staged = state
        .staged_private_pem
        .as_deref()
        .map(|pem| RsaSigningKey::from_pkcs8_pem(pem).expect("staged").jwk());
    let retired = state
        .previous
        .iter()
        .map(|entry| retired_jwk(entry).expect("retired"))
        .collect();
    SigningKeys::with_verification_keys(next, staged, retired, &SEED).expect("keys")
}

pub(crate) fn state_of(
    current: &RsaSigningKey,
    previous: Vec<PreviousSigningKey>,
) -> SigningKeyState {
    SigningKeyState {
        active_private_pem: current.to_pkcs8_pem().expect("pem"),
        staged_private_pem: None,
        previous,
    }
}

fn rotate_at(
    current: &RsaSigningKey,
    previous: Vec<PreviousSigningKey>,
    mode: RotationMode,
    now: chrono::DateTime<Utc>,
) -> (RsaSigningKey, SigningKeyState) {
    let (_, staged) = stage(&state_of(current, previous)).expect("stage");
    promote(&staged, mode, now).expect("promote")
}

fn rotate(
    current: &RsaSigningKey,
    previous: Vec<PreviousSigningKey>,
    mode: RotationMode,
) -> (RsaSigningKey, SigningKeyState) {
    rotate_at(current, previous, mode, Utc::now())
}

fn rotated(overlap: Duration) -> (RsaSigningKey, SigningKeys) {
    let old = systemprompt_test_fixtures::next_test_key();
    let (next, rotation) = rotate(&old, Vec::new(), RotationMode::Overlap(overlap));
    let keys = keys_after(next, &rotation);
    (old, keys)
}

#[test]
fn a_token_signed_by_the_retired_key_verifies_during_the_overlap() {
    let (old, keys) = rotated(Duration::hours(1));
    let claims = verify(&keys, &mint(&old)).expect("retired-key token verifies");
    assert_eq!(claims.registered.jti, "jti-rotation");
    assert_ne!(keys.active_kid(), old.kid());
}

#[test]
fn a_token_signed_by_the_new_active_key_verifies() {
    let (_, keys) = rotated(Duration::hours(1));
    let claims = verify(&keys, &mint(keys.signing_key())).expect("active-key token verifies");
    assert_eq!(claims.registered.sub, "user-1");
}

#[test]
fn a_token_signed_by_the_retired_key_fails_after_its_expiry() {
    let (old, keys) = rotated(Duration::seconds(-1));
    let err = verify(&keys, &mint(&old)).expect_err("expired retired key accepted");
    assert!(
        matches!(&err, AuthError::UnknownKid(kid) if kid == old.kid()),
        "{err}"
    );
}

#[test]
fn without_a_retired_key_the_replaced_key_is_unknown() {
    let old = systemprompt_test_fixtures::next_test_key();
    let (next, _) = rotate(&old, Vec::new(), RotationMode::Overlap(Duration::hours(1)));
    let keys = SigningKeys::new(next, &SEED).expect("keys");
    let err = verify(&keys, &mint(&old)).expect_err("replaced key accepted");
    assert!(matches!(err, AuthError::UnknownKid(_)), "{err}");
}

#[test]
fn the_jwks_publishes_the_retired_key_only_until_it_expires() {
    let (old, keys) = rotated(Duration::hours(1));
    let kids: Vec<String> = keys.jwks().keys.into_iter().map(|k| k.kid).collect();
    assert_eq!(
        kids,
        vec![keys.active_kid().to_owned(), old.kid().to_owned()]
    );

    let (_, expired) = rotated(Duration::seconds(-1));
    let kids: Vec<String> = expired.jwks().keys.into_iter().map(|k| k.kid).collect();
    assert_eq!(kids, vec![expired.active_kid().to_owned()]);
}

#[test]
fn the_rotation_retires_the_public_half_of_the_current_key() {
    let old = systemprompt_test_fixtures::next_test_key();
    let now = Utc::now();
    let (next, rotation) = rotate_at(
        &old,
        Vec::new(),
        RotationMode::Overlap(Duration::hours(2)),
        now,
    );
    let [entry] = rotation.previous.as_slice() else {
        panic!("{:?}", rotation.previous);
    };
    assert!(entry.public_pem.contains("BEGIN PUBLIC KEY"));
    assert!(!entry.public_pem.contains("PRIVATE"));
    assert_eq!(entry.expires_at, now + Duration::hours(2));
    assert_eq!(entry.kid, old.kid());
    let reloaded = RsaSigningKey::from_pkcs8_pem(&rotation.active_private_pem).expect("pem");
    assert_eq!(reloaded.kid(), next.kid());
    assert_eq!(retired_jwk(entry).expect("retired").jwk.kid, old.kid());
}

#[test]
fn the_verification_key_set_round_trips_without_private_material() {
    let (old, keys) = rotated(Duration::hours(1));
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
        staged: None,
        retired: vec![RetiredJwk {
            jwk: old.jwk(),
            expires_at: until,
        }],
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

#[test]
fn three_rotations_inside_the_overlap_leave_every_previous_key_verifying() {
    let first = systemprompt_test_fixtures::next_test_key();
    let overlap = RotationMode::Overlap(Duration::hours(1));
    let (second, rotation) = rotate(&first, Vec::new(), overlap);
    let (third, rotation) = rotate(&second, rotation.previous, overlap);
    let (fourth, rotation) = rotate(&third, rotation.previous, overlap);
    let kids: Vec<&str> = rotation.previous.iter().map(|e| e.kid.as_str()).collect();
    assert_eq!(kids, vec![first.kid(), second.kid(), third.kid()]);

    let tokens = [mint(&first), mint(&second), mint(&third)];
    let keys = keys_after(fourth, &rotation);
    for token in &tokens {
        verify(&keys, token).expect("previous-key token verifies");
    }
    assert_eq!(keys.jwks().keys.len(), 4);
}

#[test]
fn an_expired_previous_key_is_pruned_at_rotation_and_rejected_at_verification() {
    let stale = systemprompt_test_fixtures::next_test_key();
    let current = systemprompt_test_fixtures::next_test_key();
    let expired = PreviousSigningKey {
        kid: stale.kid().to_owned(),
        public_pem: stale.public_key_pem().expect("pem"),
        expires_at: Utc::now() - Duration::seconds(1),
    };

    let keys = SigningKeys::with_verification_keys(
        current.clone(),
        None,
        vec![retired_jwk(&expired).expect("retired")],
        &SEED,
    )
    .expect("keys");
    let err = verify(&keys, &mint(&stale)).expect_err("expired key accepted");
    assert!(matches!(err, AuthError::UnknownKid(_)), "{err}");
    assert!(keys.jwks().keys.iter().all(|k| k.kid != stale.kid()));

    let (_, rotation) = rotate(
        &current,
        vec![expired],
        RotationMode::Overlap(Duration::hours(1)),
    );
    let kids: Vec<&str> = rotation.previous.iter().map(|e| e.kid.as_str()).collect();
    assert_eq!(kids, vec![current.kid()]);
}

#[test]
fn a_revoking_rotation_invalidates_the_old_key_and_every_previous_key() {
    let first = systemprompt_test_fixtures::next_test_key();
    let (second, rotation) = rotate(
        &first,
        Vec::new(),
        RotationMode::Overlap(Duration::hours(1)),
    );
    let (third, rotation) = rotate(&second, rotation.previous, RotationMode::Revoke);
    assert!(rotation.previous.is_empty());

    let keys = keys_after(third, &rotation);
    for old in [&first, &second] {
        let err = verify(&keys, &mint(old)).expect_err("revoked key accepted");
        assert!(matches!(err, AuthError::UnknownKid(_)), "{err}");
    }
    assert_eq!(keys.jwks().keys.len(), 1);
    verify(&keys, &mint(keys.signing_key())).expect("new key verifies");
}

#[test]
fn a_previous_key_whose_kid_does_not_match_its_public_key_is_rejected() {
    let key = systemprompt_test_fixtures::next_test_key();
    let entry = PreviousSigningKey {
        kid: "not-the-kid".to_owned(),
        public_pem: key.public_key_pem().expect("pem"),
        expires_at: Utc::now() + Duration::hours(1),
    };
    let err = retired_jwk(&entry).expect_err("mismatched kid accepted");
    assert!(
        matches!(err, SigningKeysError::PreviousKidMismatch { .. }),
        "{err}"
    );
}
