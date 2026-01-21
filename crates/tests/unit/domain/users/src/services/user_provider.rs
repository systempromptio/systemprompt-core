//! Unit tests for UserProviderImpl and related conversions.
//!
//! Tests cover:
//! - From<User> for AuthUser conversion
//! - AuthUser field mappings
//! - is_active logic in conversion

use chrono::Utc;
use systemprompt_users::User;
use systemprompt_identifiers::UserId;
use systemprompt_traits::AuthUser;

// Helper function to create a test user
fn create_test_user() -> User {
    User {
        id: UserId::new("user-123".to_string()),
        name: "testuser".to_string(),
        email: "test@example.com".to_string(),
        full_name: Some("Test User".to_string()),
        display_name: Some("Test".to_string()),
        status: Some("active".to_string()),
        email_verified: Some(true),
        roles: vec!["user".to_string()],
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        is_bot: false,
        is_scanner: false,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    }
}

// ============================================================================
// From<User> for AuthUser Tests
// ============================================================================

mod auth_user_conversion_tests {
    use super::*;

    #[test]
    fn conversion_maps_id_correctly() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.id, "user-123");
    }

    #[test]
    fn conversion_maps_name_correctly() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.name, "testuser");
    }

    #[test]
    fn conversion_maps_email_correctly() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.email, "test@example.com");
    }

    #[test]
    fn conversion_maps_roles_correctly() {
        let mut user = create_test_user();
        user.roles = vec!["admin".to_string(), "user".to_string()];
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.roles.len(), 2);
        assert!(auth_user.roles.contains(&"admin".to_string()));
        assert!(auth_user.roles.contains(&"user".to_string()));
    }

    #[test]
    fn conversion_sets_is_active_true_for_active_status() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();

        assert!(auth_user.is_active);
    }

    #[test]
    fn conversion_sets_is_active_false_for_inactive_status() {
        let mut user = create_test_user();
        user.status = Some("inactive".to_string());
        let auth_user: AuthUser = user.into();

        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_sets_is_active_false_for_suspended_status() {
        let mut user = create_test_user();
        user.status = Some("suspended".to_string());
        let auth_user: AuthUser = user.into();

        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_sets_is_active_false_for_pending_status() {
        let mut user = create_test_user();
        user.status = Some("pending".to_string());
        let auth_user: AuthUser = user.into();

        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_sets_is_active_false_for_deleted_status() {
        let mut user = create_test_user();
        user.status = Some("deleted".to_string());
        let auth_user: AuthUser = user.into();

        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_sets_is_active_false_for_none_status() {
        let mut user = create_test_user();
        user.status = None;
        let auth_user: AuthUser = user.into();

        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_with_empty_roles() {
        let mut user = create_test_user();
        user.roles = vec![];
        let auth_user: AuthUser = user.into();

        assert!(auth_user.roles.is_empty());
    }

    #[test]
    fn conversion_with_many_roles() {
        let mut user = create_test_user();
        user.roles = vec![
            "admin".to_string(),
            "user".to_string(),
            "moderator".to_string(),
            "editor".to_string(),
            "reviewer".to_string(),
        ];
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.roles.len(), 5);
    }

    #[test]
    fn conversion_preserves_email_format() {
        let mut user = create_test_user();
        user.email = "user+tag@subdomain.example.com".to_string();
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.email, "user+tag@subdomain.example.com");
    }

    #[test]
    fn conversion_preserves_unicode_name() {
        let mut user = create_test_user();
        user.name = "用户名".to_string();
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.name, "用户名");
    }

    #[test]
    fn conversion_with_uuid_style_id() {
        let mut user = create_test_user();
        user.id = UserId::new("550e8400-e29b-41d4-a716-446655440000".to_string());
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn conversion_with_short_id() {
        let mut user = create_test_user();
        user.id = UserId::new("a".to_string());
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.id, "a");
    }
}

// ============================================================================
// AuthUser Structure Tests
// ============================================================================

mod auth_user_structure_tests {
    use super::*;

    #[test]
    fn auth_user_is_clonable() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();
        let cloned = auth_user.clone();

        assert_eq!(auth_user.id, cloned.id);
        assert_eq!(auth_user.name, cloned.name);
        assert_eq!(auth_user.email, cloned.email);
        assert_eq!(auth_user.roles, cloned.roles);
        assert_eq!(auth_user.is_active, cloned.is_active);
    }

    #[test]
    fn auth_user_is_debuggable() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();
        let debug = format!("{:?}", auth_user);

        assert!(debug.contains("AuthUser") || debug.contains("id") || debug.contains("user-123"));
    }

    #[test]
    fn auth_user_fields_are_public() {
        let user = create_test_user();
        let auth_user: AuthUser = user.into();

        // All fields should be accessible
        let _ = &auth_user.id;
        let _ = &auth_user.name;
        let _ = &auth_user.email;
        let _ = &auth_user.roles;
        let _ = auth_user.is_active;
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn conversion_with_empty_strings() {
        let user = User {
            id: UserId::new("".to_string()),
            name: "".to_string(),
            email: "".to_string(),
            full_name: None,
            display_name: None,
            status: Some("active".to_string()),
            email_verified: None,
            roles: vec![],
            avatar_url: None,
            is_bot: false,
            is_scanner: false,
            created_at: None,
            updated_at: None,
        };

        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.id, "");
        assert_eq!(auth_user.name, "");
        assert_eq!(auth_user.email, "");
        assert!(auth_user.is_active);
    }

    #[test]
    fn conversion_with_unknown_status() {
        let mut user = create_test_user();
        user.status = Some("unknown_status".to_string());
        let auth_user: AuthUser = user.into();

        // Only "active" should result in is_active = true
        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_with_uppercase_active_status() {
        let mut user = create_test_user();
        user.status = Some("ACTIVE".to_string());
        let auth_user: AuthUser = user.into();

        // Status comparison is case-sensitive
        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_with_mixed_case_active_status() {
        let mut user = create_test_user();
        user.status = Some("Active".to_string());
        let auth_user: AuthUser = user.into();

        // Status comparison is case-sensitive
        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_with_whitespace_in_status() {
        let mut user = create_test_user();
        user.status = Some(" active ".to_string());
        let auth_user: AuthUser = user.into();

        // Status with whitespace should not match
        assert!(!auth_user.is_active);
    }

    #[test]
    fn conversion_with_duplicate_roles() {
        let mut user = create_test_user();
        user.roles = vec![
            "user".to_string(),
            "user".to_string(),
            "admin".to_string(),
        ];
        let auth_user: AuthUser = user.into();

        // Duplicates are preserved (not deduplicated)
        assert_eq!(auth_user.roles.len(), 3);
    }

    #[test]
    fn conversion_with_empty_role_strings() {
        let mut user = create_test_user();
        user.roles = vec!["".to_string(), "user".to_string()];
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.roles.len(), 2);
        assert!(auth_user.roles.contains(&"".to_string()));
    }

    #[test]
    fn conversion_id_is_string_representation() {
        let user = create_test_user();
        let user_id_str = user.id.to_string();
        let auth_user: AuthUser = user.into();

        assert_eq!(auth_user.id, user_id_str);
    }
}
