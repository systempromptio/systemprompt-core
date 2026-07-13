//! `HookTokenValidator` projects the validated `plugin_id` / `sub` claims to
//! typed identifiers (`PluginId` / `UserId`), not raw strings — the contract
//! the downstream hook endpoint relies on. A token issued for plugin A must
//! also be rejected when driven against plugin B.

use std::sync::Once;

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_identifiers::{ClientId, PluginId, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::HookTokenValidator;
use systemprompt_security::keys::{RsaSigningKey, authority};

static INSTALL: Once = Once::new();

const ISSUER: &str = "hook-issuer";

fn ensure_authority() -> &'static RsaSigningKey {
    INSTALL.call_once(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("generate test signing key");
        authority::install_for_test(key);
    });
    authority::signing_key().expect("authority installed")
}

fn hook_claims(plugin_id: &str, subject: &str) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: subject.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: ISSUER.to_string(),
        aud: vec![JwtAudience::Hook],
        jti: "jti-hook".to_string(),
        scope: vec![Permission::HookGovern, Permission::HookTrack],
        username: "hook".to_string(),
        email: "hook@example.com".to_string(),
        user_type: UserType::Service,
        roles: vec!["service".to_string()],
        attributes: std::collections::BTreeMap::new(),
        client_id: Some(ClientId::new("c")),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(SessionId::new("s")),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: Some(plugin_id.to_string()),
        act: None,
    }
}

fn mint(claims: &JwtClaims) -> String {
    let key = ensure_authority();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid().to_string());
    let der = key
        .private_key()
        .to_pkcs1_der()
        .expect("encode private key");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    encode(&header, claims, &enc).expect("encode token")
}

#[test]
fn validated_govern_claims_expose_typed_ids() {
    let _ = ensure_authority();
    let token = mint(&hook_claims("plugin-x", "user-9"));

    let validator = HookTokenValidator::new(ISSUER.to_string());
    let claims = validator
        .validate_govern(&token, Some("plugin-x"))
        .expect("hook token validates");

    let plugin_id: PluginId = claims.plugin_id;
    let subject: UserId = claims.subject;
    assert_eq!(plugin_id.as_str(), "plugin-x");
    assert_eq!(subject.as_str(), "user-9");
    assert!(claims.scopes.contains(&Permission::HookGovern));
}

#[test]
fn plugin_id_mismatch_is_rejected() {
    let _ = ensure_authority();
    let token = mint(&hook_claims("plugin-a", "user-9"));

    let validator = HookTokenValidator::new(ISSUER.to_string());
    let result = validator.validate_govern(&token, Some("plugin-b"));
    assert!(
        result.is_err(),
        "token issued for plugin-a must not drive plugin-b"
    );
}

#[test]
fn validate_track_accepts_a_track_scoped_token() {
    let _ = ensure_authority();
    let token = mint(&hook_claims("plugin-x", "user-9"));

    let validator = HookTokenValidator::new(ISSUER.to_string());
    let claims = validator
        .validate_track(&token, Some("plugin-x"))
        .expect("track-scoped hook token validates");
    assert!(claims.scopes.contains(&Permission::HookTrack));
}

#[test]
fn token_without_the_required_scope_is_rejected_by_name() {
    let _ = ensure_authority();
    let mut claims = hook_claims("plugin-x", "user-9");
    claims.scope = vec![Permission::HookGovern];
    let token = mint(&claims);

    let validator = HookTokenValidator::new(ISSUER.to_string());
    let err = validator
        .validate_track(&token, Some("plugin-x"))
        .expect_err("govern-only token cannot track");
    assert!(err.to_string().contains("hook:track"), "got: {err}");
}

#[test]
fn token_without_a_plugin_id_claim_is_rejected() {
    let _ = ensure_authority();
    let mut claims = hook_claims("plugin-x", "user-9");
    claims.plugin_id = None;
    let token = mint(&claims);

    let validator = HookTokenValidator::new(ISSUER.to_string());
    let result = validator.validate_govern(&token, None);
    assert!(result.is_err(), "plugin_id claim is mandatory");
}
