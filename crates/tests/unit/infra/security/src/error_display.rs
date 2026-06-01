use systemprompt_models::auth::UserType;
use systemprompt_security::error::{AuthError, JwtError, ManifestSigningError};

#[test]
fn auth_error_missing_authorization_display() {
    let e = AuthError::MissingAuthorization;
    assert_eq!(e.to_string(), "missing authorization header");
}

#[test]
fn auth_error_missing_session_id_display() {
    let e = AuthError::MissingSessionId;
    assert_eq!(e.to_string(), "missing session_id in token");
}

#[test]
fn auth_error_hook_audience_missing_display() {
    let e = AuthError::HookAudienceMissing;
    let s = e.to_string();
    assert!(s.contains("hook token"), "got: {s}");
}

#[test]
fn auth_error_hook_scope_missing_display() {
    let e = AuthError::HookScopeMissing("read:files");
    let s = e.to_string();
    assert!(s.contains("read:files"), "got: {s}");
}

#[test]
fn auth_error_hook_plugin_id_missing_display() {
    let e = AuthError::HookPluginIdMissing;
    let s = e.to_string();
    assert!(s.contains("plugin_id"), "got: {s}");
}

#[test]
fn auth_error_hook_plugin_id_mismatch_display() {
    let e = AuthError::HookPluginIdMismatch {
        expected: "plugin-a".to_owned(),
        actual: "plugin-b".to_owned(),
    };
    let s = e.to_string();
    assert!(s.contains("plugin-a"), "got: {s}");
    assert!(s.contains("plugin-b"), "got: {s}");
}

#[test]
fn auth_error_unsupported_algorithm_display() {
    let e = AuthError::UnsupportedAlgorithm {
        got: "HS256".to_owned(),
    };
    let s = e.to_string();
    assert!(s.contains("HS256"), "got: {s}");
    assert!(s.contains("RS256"), "got: {s}");
}

#[test]
fn auth_error_missing_kid_display() {
    let e = AuthError::MissingKid;
    let s = e.to_string();
    assert!(s.contains("kid"), "got: {s}");
}

#[test]
fn auth_error_unknown_kid_display() {
    let e = AuthError::UnknownKid("kid-xyz".to_owned());
    let s = e.to_string();
    assert!(s.contains("kid-xyz"), "got: {s}");
}

#[test]
fn auth_error_untrusted_issuer_display() {
    let e = AuthError::UntrustedIssuer("evil.com".to_owned());
    let s = e.to_string();
    assert!(s.contains("evil.com"), "got: {s}");
}

#[test]
fn auth_error_act_chain_too_deep_display() {
    let e = AuthError::ActChainTooDeep { depth: 10, max: 5 };
    let s = e.to_string();
    assert!(s.contains("10"), "got: {s}");
    assert!(s.contains("5"), "got: {s}");
}

#[test]
fn auth_error_missing_scope_display() {
    let e = AuthError::MissingScope;
    let s = e.to_string();
    assert!(s.contains("scope"), "got: {s}");
}

#[test]
fn auth_error_user_type_mismatch_display() {
    let e = AuthError::UserTypeMismatch {
        claimed: UserType::Admin,
        derived: UserType::User,
    };
    let s = e.to_string();
    assert!(!s.is_empty(), "got empty display");
}

#[test]
fn jwt_error_signing_display() {
    let e = JwtError::Signing("key not found".to_owned());
    let s = e.to_string();
    assert!(s.contains("key not found"), "got: {s}");
}

#[test]
fn manifest_signing_error_seed_unavailable_display() {
    let e = ManifestSigningError::SeedUnavailable("env missing".to_owned());
    let s = e.to_string();
    assert!(s.contains("env missing"), "got: {s}");
}

#[test]
fn manifest_signing_error_canonicalize_display() {
    let e = ManifestSigningError::Canonicalize("json err".to_owned());
    let s = e.to_string();
    assert!(s.contains("json err"), "got: {s}");
}

#[test]
fn manifest_signing_error_key_missing_display() {
    let e = ManifestSigningError::KeyMissing;
    let s = e.to_string();
    assert!(s.contains("missing"), "got: {s}");
}

#[test]
fn auth_error_debug_format() {
    let e = AuthError::MissingKid;
    let d = format!("{e:?}");
    assert!(d.contains("MissingKid"), "got: {d}");
}
