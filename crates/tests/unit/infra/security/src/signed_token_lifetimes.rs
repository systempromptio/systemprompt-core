//! The lifetime bound of every token the deployment key signs, the rotation
//! overlap derived from it, and the cap each minting path enforces.

use chrono::{Duration, Utc};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_manifest::profile::SecurityConfig;
use systemprompt_models::auth::{Permission, RateLimitTier, TokenAuthority, TokenError, UserType};
use systemprompt_security::jwt::{
    AdminTokenParams, JwtService, MAX_TOKEN_LIFETIME, RotationOverlapError, SignedTokenKind,
    required_overlap, rotation_overlap, signed_token_lifetimes,
};
use systemprompt_security::session::{SessionGenerator, SessionParams};
use systemprompt_security::{DeploymentTokenAuthority, JwtError};
use systemprompt_test_fixtures::test_signing_keys;

fn security(access_secs: i64, id_jag_secs: i64) -> SecurityConfig {
    serde_json::from_value(serde_json::json!({
        "jwt_access_token_expiration": access_secs,
        "jwt_refresh_token_expiration": 2_592_000,
        "jwt_audiences": ["api"],
        "id_jag_ttl_secs": id_jag_secs,
    }))
    .expect("security config")
}

fn max_of(config: &SecurityConfig, kind: SignedTokenKind) -> Duration {
    signed_token_lifetimes(config)
        .into_iter()
        .find(|lifetime| lifetime.kind == kind)
        .expect("kind listed")
        .max
}

#[test]
fn configured_lifetimes_come_from_the_profile_fields() {
    let config = security(7_200, 120);
    assert_eq!(
        max_of(&config, SignedTokenKind::AccessToken),
        Duration::seconds(7_200)
    );
    assert_eq!(
        max_of(&config, SignedTokenKind::IdJag),
        Duration::seconds(120)
    );
    for kind in [
        SignedTokenKind::CallerLifetimeUserToken,
        SignedTokenKind::CliSessionToken,
        SignedTokenKind::ExtensionSignedClaims,
    ] {
        assert_eq!(max_of(&config, kind), MAX_TOKEN_LIFETIME);
    }
}

#[test]
fn the_default_overlap_is_the_longest_signed_token_lifetime() {
    let config = security(7_200, 120);
    let lifetimes = signed_token_lifetimes(&config);
    let longest = lifetimes.iter().map(|l| l.max).max().expect("non-empty");
    assert_eq!(required_overlap(&lifetimes), longest);
    assert_eq!(
        rotation_overlap(&config, None, false).expect("default"),
        longest
    );
}

#[test]
fn a_configured_lifetime_above_the_cap_is_bounded_by_the_cap() {
    let config = security(i64::from(i32::MAX), 120);
    assert_eq!(
        max_of(&config, SignedTokenKind::AccessToken),
        MAX_TOKEN_LIFETIME
    );
}

#[test]
fn a_short_explicit_overlap_is_refused_naming_the_cut_off_tokens() {
    let config = security(7_200, 120);
    let err = rotation_overlap(&config, Some(Duration::seconds(3_600)), false)
        .expect_err("short overlap accepted");
    let RotationOverlapError::CutsOff { cut_off, .. } = &err else {
        panic!("{err}");
    };
    let kinds: Vec<SignedTokenKind> = cut_off.iter().map(|l| l.kind).collect();
    assert!(kinds.contains(&SignedTokenKind::AccessToken), "{kinds:?}");
    assert!(!kinds.contains(&SignedTokenKind::IdJag), "{kinds:?}");
    let text = err.to_string();
    assert!(
        text.contains("security.jwt_access_token_expiration"),
        "{text}"
    );
    assert!(text.contains("--force"), "{text}");
}

#[test]
fn force_accepts_a_short_overlap() {
    let config = security(7_200, 120);
    let overlap = rotation_overlap(&config, Some(Duration::seconds(60)), true).expect("forced");
    assert_eq!(overlap, Duration::seconds(60));
}

#[test]
fn an_overlap_covering_every_lifetime_is_accepted_without_force() {
    let config = security(7_200, 120);
    assert_eq!(
        rotation_overlap(&config, Some(MAX_TOKEN_LIFETIME), false).expect("covering"),
        MAX_TOKEN_LIFETIME
    );
}

#[test]
fn a_non_positive_overlap_is_refused_even_when_forced() {
    let config = security(7_200, 120);
    let err = rotation_overlap(&config, Some(Duration::zero()), true).expect_err("zero accepted");
    assert!(
        matches!(err, RotationOverlapError::NotPositive { seconds: 0 }),
        "{err}"
    );
}

#[test]
fn a_session_token_beyond_the_cap_is_refused() {
    let uid = UserId::new("cap-user");
    let sid = SessionId::generate();
    let params = SessionParams {
        user_id: &uid,
        session_id: &sid,
        email: "cap@example.com",
        duration: MAX_TOKEN_LIFETIME + Duration::days(1),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_owned()],
        attributes: Default::default(),
        rate_limit_tier: RateLimitTier::User,
    };
    let err = SessionGenerator::new("issuer", test_signing_keys())
        .generate(&params)
        .expect_err("uncapped session token minted");
    assert!(matches!(err, JwtError::InvalidLifetime { .. }), "{err}");
}

#[test]
fn an_admin_token_beyond_the_cap_is_refused() {
    let uid = UserId::new("cap-admin");
    let sid = SessionId::generate();
    let err = JwtService::generate_admin_token(
        &test_signing_keys(),
        &AdminTokenParams {
            user_id: &uid,
            session_id: &sid,
            email: "admin@example.com",
            issuer: "issuer",
            duration: MAX_TOKEN_LIFETIME + Duration::days(1),
            client_id: None,
        },
    )
    .expect_err("uncapped admin token minted");
    assert!(matches!(err, JwtError::InvalidLifetime { .. }), "{err}");
}

#[test]
fn extension_claims_without_a_bounded_expiry_are_refused() {
    let authority = DeploymentTokenAuthority::new(test_signing_keys(), "issuer");
    let now = Utc::now().timestamp();
    for claims in [
        serde_json::json!({ "sub": "x" }),
        serde_json::json!({ "sub": "x", "exp": now + MAX_TOKEN_LIFETIME.num_seconds() + 60 }),
    ] {
        let err = authority
            .sign_claims(&claims)
            .expect_err("unbounded claims signed");
        assert!(matches!(err, TokenError::InvalidLifetime { .. }), "{err}");
    }
    authority
        .sign_claims(&serde_json::json!({ "sub": "x", "exp": now + 600 }))
        .expect("bounded claims sign");
}
