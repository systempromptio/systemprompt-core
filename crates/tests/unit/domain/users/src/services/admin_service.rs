//! Unit tests for UserAdminService types.
//!
//! Tests cover:
//! - PromoteResult enum variants
//! - DemoteResult enum variants
//! - Result type patterns

use chrono::Utc;
use systemprompt_core_users::{DemoteResult, PromoteResult, User};
use systemprompt_identifiers::UserId;

// Helper function to create a test user
fn create_test_user(roles: Vec<String>) -> User {
    User {
        id: UserId::new("user-123".to_string()),
        name: "testuser".to_string(),
        email: "test@example.com".to_string(),
        full_name: Some("Test User".to_string()),
        display_name: Some("Test".to_string()),
        status: Some("active".to_string()),
        email_verified: Some(true),
        roles,
        avatar_url: None,
        is_bot: false,
        is_scanner: false,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    }
}

// ============================================================================
// PromoteResult Tests
// ============================================================================

mod promote_result_tests {
    use super::*;

    #[test]
    fn promoted_variant_contains_user_and_roles() {
        let user = create_test_user(vec!["admin".to_string(), "user".to_string()]);
        let new_roles = vec!["admin".to_string(), "user".to_string()];
        let result = PromoteResult::Promoted(user.clone(), new_roles.clone());

        match result {
            PromoteResult::Promoted(u, roles) => {
                assert_eq!(u.id.to_string(), user.id.to_string());
                assert_eq!(roles, new_roles);
            }
            _ => panic!("Expected Promoted variant"),
        }
    }

    #[test]
    fn already_admin_variant_contains_user() {
        let user = create_test_user(vec!["admin".to_string()]);
        let result = PromoteResult::AlreadyAdmin(user.clone());

        match result {
            PromoteResult::AlreadyAdmin(u) => {
                assert_eq!(u.id.to_string(), user.id.to_string());
                assert!(u.roles.contains(&"admin".to_string()));
            }
            _ => panic!("Expected AlreadyAdmin variant"),
        }
    }

    #[test]
    fn user_not_found_variant() {
        let result: PromoteResult = PromoteResult::UserNotFound;

        assert!(matches!(result, PromoteResult::UserNotFound));
    }

    #[test]
    fn promote_result_debug_promoted() {
        let user = create_test_user(vec!["admin".to_string()]);
        let result = PromoteResult::Promoted(user, vec!["admin".to_string()]);

        let debug = format!("{:?}", result);
        assert!(debug.contains("Promoted"));
    }

    #[test]
    fn promote_result_debug_already_admin() {
        let user = create_test_user(vec!["admin".to_string()]);
        let result = PromoteResult::AlreadyAdmin(user);

        let debug = format!("{:?}", result);
        assert!(debug.contains("AlreadyAdmin"));
    }

    #[test]
    fn promote_result_debug_user_not_found() {
        let result = PromoteResult::UserNotFound;

        let debug = format!("{:?}", result);
        assert!(debug.contains("UserNotFound"));
    }

    #[test]
    fn promoted_with_multiple_roles() {
        let user = create_test_user(vec![
            "admin".to_string(),
            "user".to_string(),
            "moderator".to_string(),
        ]);
        let new_roles = vec![
            "admin".to_string(),
            "user".to_string(),
            "moderator".to_string(),
        ];
        let result = PromoteResult::Promoted(user, new_roles.clone());

        match result {
            PromoteResult::Promoted(_, roles) => {
                assert_eq!(roles.len(), 3);
                assert!(roles.contains(&"admin".to_string()));
                assert!(roles.contains(&"user".to_string()));
                assert!(roles.contains(&"moderator".to_string()));
            }
            _ => panic!("Expected Promoted variant"),
        }
    }

    #[test]
    fn promoted_with_empty_roles() {
        let user = create_test_user(vec![]);
        let result = PromoteResult::Promoted(user, vec!["admin".to_string()]);

        match result {
            PromoteResult::Promoted(_, roles) => {
                assert_eq!(roles.len(), 1);
            }
            _ => panic!("Expected Promoted variant"),
        }
    }
}

// ============================================================================
// DemoteResult Tests
// ============================================================================

mod demote_result_tests {
    use super::*;

    #[test]
    fn demoted_variant_contains_user_and_roles() {
        let user = create_test_user(vec!["user".to_string()]);
        let new_roles = vec!["user".to_string()];
        let result = DemoteResult::Demoted(user.clone(), new_roles.clone());

        match result {
            DemoteResult::Demoted(u, roles) => {
                assert_eq!(u.id.to_string(), user.id.to_string());
                assert_eq!(roles, new_roles);
            }
            _ => panic!("Expected Demoted variant"),
        }
    }

    #[test]
    fn not_admin_variant_contains_user() {
        let user = create_test_user(vec!["user".to_string()]);
        let result = DemoteResult::NotAdmin(user.clone());

        match result {
            DemoteResult::NotAdmin(u) => {
                assert_eq!(u.id.to_string(), user.id.to_string());
                assert!(!u.roles.contains(&"admin".to_string()));
            }
            _ => panic!("Expected NotAdmin variant"),
        }
    }

    #[test]
    fn user_not_found_variant() {
        let result: DemoteResult = DemoteResult::UserNotFound;

        assert!(matches!(result, DemoteResult::UserNotFound));
    }

    #[test]
    fn demote_result_debug_demoted() {
        let user = create_test_user(vec!["user".to_string()]);
        let result = DemoteResult::Demoted(user, vec!["user".to_string()]);

        let debug = format!("{:?}", result);
        assert!(debug.contains("Demoted"));
    }

    #[test]
    fn demote_result_debug_not_admin() {
        let user = create_test_user(vec!["user".to_string()]);
        let result = DemoteResult::NotAdmin(user);

        let debug = format!("{:?}", result);
        assert!(debug.contains("NotAdmin"));
    }

    #[test]
    fn demote_result_debug_user_not_found() {
        let result = DemoteResult::UserNotFound;

        let debug = format!("{:?}", result);
        assert!(debug.contains("UserNotFound"));
    }

    #[test]
    fn demoted_removes_admin_role() {
        let user = create_test_user(vec!["user".to_string()]);
        let remaining_roles = vec!["user".to_string()];
        let result = DemoteResult::Demoted(user, remaining_roles.clone());

        match result {
            DemoteResult::Demoted(_, roles) => {
                assert!(!roles.contains(&"admin".to_string()));
                assert!(roles.contains(&"user".to_string()));
            }
            _ => panic!("Expected Demoted variant"),
        }
    }

    #[test]
    fn demoted_keeps_other_roles() {
        let user = create_test_user(vec!["user".to_string(), "moderator".to_string()]);
        let remaining_roles = vec!["user".to_string(), "moderator".to_string()];
        let result = DemoteResult::Demoted(user, remaining_roles);

        match result {
            DemoteResult::Demoted(_, roles) => {
                assert_eq!(roles.len(), 2);
                assert!(roles.contains(&"user".to_string()));
                assert!(roles.contains(&"moderator".to_string()));
            }
            _ => panic!("Expected Demoted variant"),
        }
    }
}

// ============================================================================
// UpdateUserParams Tests
// ============================================================================

mod update_user_params_tests {
    use systemprompt_core_users::{UpdateUserParams, UserStatus};

    #[test]
    fn update_user_params_creation() {
        let params = UpdateUserParams {
            email: "newemail@example.com",
            full_name: Some("New Full Name"),
            display_name: Some("New Display"),
            status: UserStatus::Active,
        };

        assert_eq!(params.email, "newemail@example.com");
        assert_eq!(params.full_name, Some("New Full Name"));
        assert_eq!(params.display_name, Some("New Display"));
        assert_eq!(params.status, UserStatus::Active);
    }

    #[test]
    fn update_user_params_with_none_fields() {
        let params = UpdateUserParams {
            email: "test@example.com",
            full_name: None,
            display_name: None,
            status: UserStatus::Pending,
        };

        assert!(params.full_name.is_none());
        assert!(params.display_name.is_none());
    }

    #[test]
    fn update_user_params_with_suspended_status() {
        let params = UpdateUserParams {
            email: "test@example.com",
            full_name: None,
            display_name: None,
            status: UserStatus::Suspended,
        };

        assert_eq!(params.status, UserStatus::Suspended);
    }

    #[test]
    fn update_user_params_with_inactive_status() {
        let params = UpdateUserParams {
            email: "test@example.com",
            full_name: None,
            display_name: None,
            status: UserStatus::Inactive,
        };

        assert_eq!(params.status, UserStatus::Inactive);
    }

    #[test]
    fn update_user_params_debug() {
        let params = UpdateUserParams {
            email: "test@example.com",
            full_name: Some("Full Name"),
            display_name: None,
            status: UserStatus::Active,
        };

        let debug = format!("{:?}", params);
        assert!(debug.contains("UpdateUserParams"));
        assert!(debug.contains("test@example.com"));
    }

    #[test]
    fn update_user_params_all_statuses() {
        let statuses = [
            UserStatus::Active,
            UserStatus::Inactive,
            UserStatus::Suspended,
            UserStatus::Pending,
            UserStatus::Deleted,
            UserStatus::Temporary,
        ];

        for status in statuses {
            let params = UpdateUserParams {
                email: "test@example.com",
                full_name: None,
                display_name: None,
                status,
            };

            assert_eq!(params.status, status);
        }
    }

    #[test]
    fn update_user_params_with_empty_email() {
        let params = UpdateUserParams {
            email: "",
            full_name: None,
            display_name: None,
            status: UserStatus::Active,
        };

        assert_eq!(params.email, "");
    }

    #[test]
    fn update_user_params_with_special_characters() {
        let params = UpdateUserParams {
            email: "user+tag@sub.domain.com",
            full_name: Some("José García"),
            display_name: Some("José"),
            status: UserStatus::Active,
        };

        assert_eq!(params.email, "user+tag@sub.domain.com");
        assert_eq!(params.full_name, Some("José García"));
    }
}
