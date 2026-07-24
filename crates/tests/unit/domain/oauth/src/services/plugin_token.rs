//! Tests for `PluginTokenService`: mint a plugin-scoped JWT against the
//! test signing authority, decode it, and assert the claim shape.

use base64::Engine;
use jsonwebtoken::{Algorithm, decode_header};
use systemprompt_identifiers::SessionId;
use systemprompt_models::auth::{JwtAudience, JwtClaims, Permission};
use systemprompt_oauth::services::plugin_token::{PluginTokenService, PluginTokenSubject};
use systemprompt_test_fixtures::install_test_signing_key;

const ISSUER: &str = "https://issuer.test";

fn subject() -> PluginTokenSubject {
    PluginTokenSubject {
        id: uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").expect("uuid"),
        username: "Admin User".to_owned(),
        email: "admin@example.com".to_owned(),
    }
}

fn decode_claims(token: &str) -> JwtClaims {
    let payload = token.split('.').nth(1).expect("jwt payload segment");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("base64-decode payload");
    serde_json::from_slice(&bytes).expect("decode minted token")
}

#[test]
fn issue_mints_rs256_token_with_kid_header() {
    install_test_signing_key();

    let issued = PluginTokenService::issue(
        subject(),
        ISSUER,
        "cowork-bundle".to_owned(),
        30,
        &SessionId::generate(),
    )
    .expect("issue plugin token");

    let header = decode_header(&issued.token).expect("decode header");
    assert_eq!(header.alg, Algorithm::RS256);
    let kid = header.kid.expect("kid header present");
    assert!(!kid.is_empty(), "kid identifies the signing key");
}

#[test]
fn issue_embeds_hook_scope_plugin_audience_and_plugin_id() {
    install_test_signing_key();

    let session_id = SessionId::generate();
    let issued = PluginTokenService::issue(
        subject(),
        ISSUER,
        "cowork-bundle".to_owned(),
        30,
        &session_id,
    )
    .expect("issue plugin token");
    let claims = decode_claims(&issued.token);

    assert_eq!(
        claims.session_id.as_ref(),
        Some(&session_id),
        "the caller's persisted session must reach the claim, or the governance webhook cannot attest it"
    );

    assert_eq!(claims.sub, "11111111-2222-3333-4444-555555555555");
    assert_eq!(claims.iss, ISSUER);
    assert_eq!(claims.jti, issued.jti);
    assert_eq!(claims.username, "Admin User");
    assert_eq!(claims.email, "admin@example.com");
    assert_eq!(
        claims.scope,
        vec![Permission::HookGovern, Permission::HookTrack]
    );
    assert_eq!(
        claims.aud,
        vec![
            JwtAudience::Hook,
            JwtAudience::Resource("plugin".to_owned())
        ]
    );
    assert_eq!(claims.plugin_id.as_deref(), Some("cowork-bundle"));
    assert!(claims.roles.is_empty());
}

#[test]
fn issue_sets_expiry_from_duration_days() {
    install_test_signing_key();

    let issued = PluginTokenService::issue(
        subject(),
        ISSUER,
        "cowork-bundle".to_owned(),
        30,
        &SessionId::generate(),
    )
    .expect("issue plugin token");
    let claims = decode_claims(&issued.token);

    // The service computes `exp` from a clock read taken just before `iat`,
    // so the difference may undershoot the full window by a second or two.
    let lifetime = claims.exp - claims.iat;
    let expected = 30 * 24 * 3600;
    assert!((expected - 2..=expected).contains(&lifetime));
}

#[test]
fn issue_rejects_duration_beyond_one_year() {
    install_test_signing_key();

    PluginTokenService::issue(
        subject(),
        ISSUER,
        "cowork-bundle".to_owned(),
        366,
        &SessionId::generate(),
    )
    .expect_err("366 days exceeds the 8760-hour expiry ceiling");
}

#[test]
fn issue_generates_unique_jti_per_token() {
    install_test_signing_key();

    let first = PluginTokenService::issue(
        subject(),
        ISSUER,
        "cowork-bundle".to_owned(),
        30,
        &SessionId::generate(),
    )
    .expect("first token");
    let second = PluginTokenService::issue(
        subject(),
        ISSUER,
        "cowork-bundle".to_owned(),
        30,
        &SessionId::generate(),
    )
    .expect("second token");

    assert_ne!(first.jti, second.jti);
    assert_ne!(first.token, second.token);
}
