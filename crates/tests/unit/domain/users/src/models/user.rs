//! Unit tests for User struct helper methods

use chrono::Utc;
use systemprompt_users::{User, UserRole};
use systemprompt_identifiers::UserId;

mod user_tests {
    use super::*;

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

    #[test]
    fn is_active_returns_true_for_active_status() {
        let user = create_test_user();
        assert!(user.is_active());
    }

    #[test]
    fn is_active_returns_false_for_inactive_status() {
        let mut user = create_test_user();
        user.status = Some("inactive".to_string());
        assert!(!user.is_active());
    }

    #[test]
    fn is_active_returns_false_for_suspended_status() {
        let mut user = create_test_user();
        user.status = Some("suspended".to_string());
        assert!(!user.is_active());
    }

    #[test]
    fn is_active_returns_false_for_none_status() {
        let mut user = create_test_user();
        user.status = None;
        assert!(!user.is_active());
    }

    #[test]
    fn is_admin_returns_true_when_has_admin_role() {
        let mut user = create_test_user();
        user.roles = vec!["admin".to_string(), "user".to_string()];
        assert!(user.is_admin());
    }

    #[test]
    fn is_admin_returns_false_when_no_admin_role() {
        let user = create_test_user();
        assert!(!user.is_admin());
    }

    #[test]
    fn is_admin_returns_false_for_empty_roles() {
        let mut user = create_test_user();
        user.roles = vec![];
        assert!(!user.is_admin());
    }

    #[test]
    fn has_role_returns_true_for_existing_role() {
        let user = create_test_user();
        assert!(user.has_role(UserRole::User));
    }

    #[test]
    fn has_role_returns_false_for_missing_role() {
        let user = create_test_user();
        assert!(!user.has_role(UserRole::Admin));
    }

    #[test]
    fn has_role_returns_true_for_admin_role() {
        let mut user = create_test_user();
        user.roles = vec!["admin".to_string()];
        assert!(user.has_role(UserRole::Admin));
    }

    #[test]
    fn has_role_returns_true_for_anonymous_role() {
        let mut user = create_test_user();
        user.roles = vec!["anonymous".to_string()];
        assert!(user.has_role(UserRole::Anonymous));
    }

    #[test]
    fn has_role_returns_false_for_empty_roles() {
        let mut user = create_test_user();
        user.roles = vec![];
        assert!(!user.has_role(UserRole::User));
    }

    #[test]
    fn user_clone() {
        let user = create_test_user();
        let cloned = user.clone();
        assert_eq!(user.id.to_string(), cloned.id.to_string());
        assert_eq!(user.name, cloned.name);
        assert_eq!(user.email, cloned.email);
    }

    #[test]
    fn user_debug() {
        let user = create_test_user();
        let debug_str = format!("{:?}", user);
        assert!(debug_str.contains("User"));
        assert!(debug_str.contains("testuser"));
    }

    #[test]
    fn user_with_multiple_roles() {
        let mut user = create_test_user();
        user.roles = vec![
            "admin".to_string(),
            "user".to_string(),
            "moderator".to_string(),
        ];

        assert!(user.has_role(UserRole::Admin));
        assert!(user.has_role(UserRole::User));
        assert!(user.is_admin());
    }

    #[test]
    fn user_is_bot_field() {
        let mut user = create_test_user();
        assert!(!user.is_bot);

        user.is_bot = true;
        assert!(user.is_bot);
    }

    #[test]
    fn user_is_scanner_field() {
        let mut user = create_test_user();
        assert!(!user.is_scanner);

        user.is_scanner = true;
        assert!(user.is_scanner);
    }

    #[test]
    fn user_optional_fields_can_be_none() {
        let user = User {
            id: UserId::new("user-456".to_string()),
            name: "minimal".to_string(),
            email: "minimal@example.com".to_string(),
            full_name: None,
            display_name: None,
            status: None,
            email_verified: None,
            roles: vec![],
            avatar_url: None,
            is_bot: false,
            is_scanner: false,
            created_at: None,
            updated_at: None,
        };

        assert!(user.full_name.is_none());
        assert!(user.display_name.is_none());
        assert!(user.status.is_none());
        assert!(user.email_verified.is_none());
        assert!(user.avatar_url.is_none());
        assert!(user.created_at.is_none());
        assert!(user.updated_at.is_none());
    }
}
