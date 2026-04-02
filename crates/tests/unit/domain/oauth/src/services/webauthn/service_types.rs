//! Tests for WebAuthn service data types: VerifiedAuthentication, LinkUserInfo, LinkStates, WebAuthnManager

use std::time::Instant;
use systemprompt_oauth::services::webauthn::service::{
    LinkUserInfo, create_link_states,
};
use systemprompt_oauth::services::webauthn::service::VerifiedAuthentication;
use systemprompt_oauth::services::webauthn::WebAuthnManager;

// ============================================================================
// VerifiedAuthentication Tests
// ============================================================================

#[test]
fn test_verified_authentication_construction() {
    let auth = VerifiedAuthentication {
        user_id: "user-abc-123".to_string(),
        timestamp: Instant::now(),
    };

    assert_eq!(auth.user_id, "user-abc-123");
}

#[test]
fn test_verified_authentication_clone() {
    let original = VerifiedAuthentication {
        user_id: "user-clone-test".to_string(),
        timestamp: Instant::now(),
    };

    let cloned = original.clone();
    assert_eq!(cloned.user_id, original.user_id);
    assert_eq!(cloned.timestamp, original.timestamp);
}

#[test]
fn test_verified_authentication_debug() {
    let auth = VerifiedAuthentication {
        user_id: "debug-user-id".to_string(),
        timestamp: Instant::now(),
    };

    let debug_output = format!("{auth:?}");
    assert!(debug_output.contains("VerifiedAuthentication"));
    assert!(debug_output.contains("debug-user-id"));
}

#[test]
fn test_verified_authentication_timestamp_preserves_value() {
    let before = Instant::now();
    let auth = VerifiedAuthentication {
        user_id: "timestamp-test".to_string(),
        timestamp: before,
    };
    let after = Instant::now();

    assert!(auth.timestamp >= before);
    assert!(auth.timestamp <= after);
}

// ============================================================================
// LinkUserInfo Tests
// ============================================================================

#[test]
fn test_link_user_info_construction() {
    let info = LinkUserInfo {
        id: "user-id-456".to_string(),
        email: "test@example.com".to_string(),
        name: "Test User".to_string(),
    };

    assert_eq!(info.id, "user-id-456");
    assert_eq!(info.email, "test@example.com");
    assert_eq!(info.name, "Test User");
}

#[test]
fn test_link_user_info_clone() {
    let original = LinkUserInfo {
        id: "clone-id".to_string(),
        email: "clone@example.com".to_string(),
        name: "Clone User".to_string(),
    };

    let cloned = original.clone();
    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.email, original.email);
    assert_eq!(cloned.name, original.name);
}

#[test]
fn test_link_user_info_debug() {
    let info = LinkUserInfo {
        id: "dbg-id".to_string(),
        email: "dbg@example.com".to_string(),
        name: "Debug Name".to_string(),
    };

    let debug_output = format!("{info:?}");
    assert!(debug_output.contains("LinkUserInfo"));
    assert!(debug_output.contains("dbg-id"));
    assert!(debug_output.contains("dbg@example.com"));
    assert!(debug_output.contains("Debug Name"));
}

#[test]
fn test_link_user_info_empty_fields() {
    let info = LinkUserInfo {
        id: String::new(),
        email: String::new(),
        name: String::new(),
    };

    assert!(info.id.is_empty());
    assert!(info.email.is_empty());
    assert!(info.name.is_empty());
}

// ============================================================================
// create_link_states Tests
// ============================================================================

#[tokio::test]
async fn test_create_link_states_empty() {
    let states = create_link_states();
    let locked = states.lock().await;

    assert!(locked.is_empty());
}

#[tokio::test]
async fn test_create_link_states_is_arc() {
    let states = create_link_states();
    let cloned = states.clone();

    assert!(states.lock().await.is_empty());
    assert!(cloned.lock().await.is_empty());
}

#[tokio::test]
async fn test_create_link_states_mutex_lockable() {
    let states = create_link_states();

    {
        let guard = states.lock().await;
        assert_eq!(guard.len(), 0);
    }

    let guard = states.lock().await;
    assert_eq!(guard.len(), 0);
}

// ============================================================================
// WebAuthnManager Tests
// ============================================================================

#[test]
fn test_webauthn_manager_debug() {
    let manager = WebAuthnManager;
    let debug_output = format!("{manager:?}");

    assert!(debug_output.contains("WebAuthnManager"));
}

#[test]
fn test_webauthn_manager_clone() {
    let manager = WebAuthnManager;
    let cloned = manager.clone();
    let debug_original = format!("{manager:?}");
    let debug_cloned = format!("{cloned:?}");

    assert_eq!(debug_original, debug_cloned);
}

#[test]
fn test_webauthn_manager_copy() {
    let manager = WebAuthnManager;
    let copied = manager;
    let still_valid = manager;

    let debug_copied = format!("{copied:?}");
    let debug_still_valid = format!("{still_valid:?}");
    assert_eq!(debug_copied, debug_still_valid);
}
