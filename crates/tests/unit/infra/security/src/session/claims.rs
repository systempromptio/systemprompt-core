//! Unit tests for ValidatedSessionClaims
//!
//! Tests cover:
//! - Struct creation and field access
//! - Debug and Clone implementations

use systemprompt_models::auth::UserType;
use systemprompt_security::ValidatedSessionClaims;

#[test]
fn test_validated_session_claims_creation() {
    let claims = ValidatedSessionClaims {
        user_id: "user_123".to_string(),
        session_id: "session_456".to_string(),
        user_type: UserType::User,
    };

    assert_eq!(claims.user_id, "user_123");
    assert_eq!(claims.session_id, "session_456");
    assert_eq!(claims.user_type, UserType::User);
}

#[test]
fn test_validated_session_claims_admin_user_type() {
    let claims = ValidatedSessionClaims {
        user_id: "admin_user".to_string(),
        session_id: "admin_session".to_string(),
        user_type: UserType::Admin,
    };

    assert_eq!(claims.user_type, UserType::Admin);
}

#[test]
fn test_validated_session_claims_anon_user_type() {
    let claims = ValidatedSessionClaims {
        user_id: "anonymous".to_string(),
        session_id: "anon_session".to_string(),
        user_type: UserType::Anon,
    };

    assert_eq!(claims.user_type, UserType::Anon);
}

#[test]
fn test_validated_session_claims_debug() {
    let claims = ValidatedSessionClaims {
        user_id: "user".to_string(),
        session_id: "session".to_string(),
        user_type: UserType::User,
    };

    let debug_str = format!("{:?}", claims);
    assert!(debug_str.contains("ValidatedSessionClaims"));
    assert!(debug_str.contains("user"));
    assert!(debug_str.contains("session"));
}

#[test]
fn test_validated_session_claims_clone() {
    let original = ValidatedSessionClaims {
        user_id: "user_original".to_string(),
        session_id: "session_original".to_string(),
        user_type: UserType::User,
    };

    let cloned = original.clone();

    assert_eq!(cloned.user_id, original.user_id);
    assert_eq!(cloned.session_id, original.session_id);
    assert_eq!(cloned.user_type, original.user_type);
}

#[test]
fn test_validated_session_claims_clone_independence() {
    let original = ValidatedSessionClaims {
        user_id: "user".to_string(),
        session_id: "session".to_string(),
        user_type: UserType::User,
    };

    let mut cloned = original.clone();
    cloned.user_id = "modified_user".to_string();

    assert_eq!(original.user_id, "user");
    assert_eq!(cloned.user_id, "modified_user");
}

#[test]
fn test_validated_session_claims_empty_strings() {
    let claims = ValidatedSessionClaims {
        user_id: String::new(),
        session_id: String::new(),
        user_type: UserType::Anon,
    };

    assert!(claims.user_id.is_empty());
    assert!(claims.session_id.is_empty());
}

#[test]
fn test_validated_session_claims_uuid_format() {
    let claims = ValidatedSessionClaims {
        user_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        session_id: "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string(),
        user_type: UserType::User,
    };

    assert_eq!(claims.user_id.len(), 36);
    assert_eq!(claims.session_id.len(), 36);
}

#[test]
fn test_validated_session_claims_special_characters() {
    let claims = ValidatedSessionClaims {
        user_id: "user_with-special.chars@domain".to_string(),
        session_id: "session:with:colons".to_string(),
        user_type: UserType::User,
    };

    assert!(claims.user_id.contains('@'));
    assert!(claims.session_id.contains(':'));
}
