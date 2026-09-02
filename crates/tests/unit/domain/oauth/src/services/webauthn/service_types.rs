//! Tests for WebAuthn service data types: LinkUserInfo, WebAuthnRegistry

use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use systemprompt_oauth::services::webauthn::service::LinkUserInfo;

#[test]
fn test_link_user_info_construction() {
    let info = LinkUserInfo {
        id: "user-id-456".to_string().into(),
        email: "test@example.com".to_string(),
        name: "Test User".to_string(),
    };

    assert_eq!(info.id.as_str(), "user-id-456");
    assert_eq!(info.email, "test@example.com");
    assert_eq!(info.name, "Test User");
}

#[test]
fn test_link_user_info_clone() {
    let original = LinkUserInfo {
        id: "clone-id".to_string().into(),
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
        id: "dbg-id".to_string().into(),
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
        id: String::new().into(),
        email: String::new(),
        name: String::new(),
    };

    assert!(info.id.as_str().is_empty());
    assert!(info.email.is_empty());
    assert!(info.name.is_empty());
}

#[test]
fn test_webauthn_manager_debug() {
    let manager = WebAuthnRegistry;
    let debug_output = format!("{manager:?}");

    assert!(debug_output.contains("WebAuthnRegistry"));
}

#[test]
fn test_webauthn_manager_clone() {
    let manager = WebAuthnRegistry;
    let cloned = manager.clone();
    let debug_original = format!("{manager:?}");
    let debug_cloned = format!("{cloned:?}");

    assert_eq!(debug_original, debug_cloned);
}

#[test]
fn test_webauthn_manager_copy() {
    let manager = WebAuthnRegistry;
    let copied = manager;
    let still_valid = manager;

    let debug_copied = format!("{copied:?}");
    let debug_still_valid = format!("{still_valid:?}");
    assert_eq!(debug_copied, debug_still_valid);
}
