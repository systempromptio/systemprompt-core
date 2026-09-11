//! Tests for WebAuthn service data types: LinkUserInfo, WebAuthnRegistry

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
