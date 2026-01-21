//! Unit tests for user error types.
//!
//! Tests cover:
//! - UserError enum variants
//! - Error display messages
//! - Error conversions (From implementations)
//! - Result type alias

use systemprompt_users::UserError;
use systemprompt_identifiers::UserId;

// ============================================================================
// UserError Display Tests
// ============================================================================

mod user_error_display_tests {
    use super::*;

    #[test]
    fn not_found_displays_user_id() {
        let user_id = UserId::new("user-123".to_string());
        let error = UserError::NotFound(user_id);

        let display = error.to_string();
        assert!(display.contains("User not found"));
        assert!(display.contains("user-123"));
    }

    #[test]
    fn email_already_exists_displays_email() {
        let error = UserError::EmailAlreadyExists("test@example.com".to_string());

        let display = error.to_string();
        assert!(display.contains("User already exists"));
        assert!(display.contains("test@example.com"));
    }

    #[test]
    fn invalid_status_displays_status() {
        let error = UserError::InvalidStatus("unknown_status".to_string());

        let display = error.to_string();
        assert!(display.contains("Invalid status"));
        assert!(display.contains("unknown_status"));
    }

    #[test]
    fn invalid_role_displays_role() {
        let error = UserError::InvalidRole("superuser".to_string());

        let display = error.to_string();
        assert!(display.contains("Invalid role"));
        assert!(display.contains("superuser"));
    }

    #[test]
    fn validation_error_displays_message() {
        let error = UserError::Validation("Name cannot be empty".to_string());

        let display = error.to_string();
        assert!(display.contains("Validation error"));
        assert!(display.contains("Name cannot be empty"));
    }
}

// ============================================================================
// UserError Debug Tests
// ============================================================================

mod user_error_debug_tests {
    use super::*;

    #[test]
    fn not_found_debug() {
        let user_id = UserId::new("user-456".to_string());
        let error = UserError::NotFound(user_id);

        let debug = format!("{:?}", error);
        assert!(debug.contains("NotFound"));
    }

    #[test]
    fn email_already_exists_debug() {
        let error = UserError::EmailAlreadyExists("test@example.com".to_string());

        let debug = format!("{:?}", error);
        assert!(debug.contains("EmailAlreadyExists"));
    }

    #[test]
    fn invalid_status_debug() {
        let error = UserError::InvalidStatus("bad".to_string());

        let debug = format!("{:?}", error);
        assert!(debug.contains("InvalidStatus"));
    }

    #[test]
    fn invalid_role_debug() {
        let error = UserError::InvalidRole("bad".to_string());

        let debug = format!("{:?}", error);
        assert!(debug.contains("InvalidRole"));
    }

    #[test]
    fn validation_debug() {
        let error = UserError::Validation("error".to_string());

        let debug = format!("{:?}", error);
        assert!(debug.contains("Validation"));
    }
}

// ============================================================================
// UserError Variant Construction Tests
// ============================================================================

mod user_error_construction_tests {
    use super::*;

    #[test]
    fn not_found_with_different_user_ids() {
        let id1 = UserId::new("user-1".to_string());
        let id2 = UserId::new("user-2".to_string());

        let error1 = UserError::NotFound(id1);
        let error2 = UserError::NotFound(id2);

        assert!(error1.to_string().contains("user-1"));
        assert!(error2.to_string().contains("user-2"));
    }

    #[test]
    fn email_already_exists_with_various_emails() {
        let emails = [
            "simple@example.com",
            "complex.name+tag@subdomain.example.com",
            "user@localhost",
        ];

        for email in emails {
            let error = UserError::EmailAlreadyExists(email.to_string());
            assert!(error.to_string().contains(email));
        }
    }

    #[test]
    fn invalid_status_with_various_values() {
        let statuses = ["", "unknown", "ACTIVE", "123"];

        for status in statuses {
            let error = UserError::InvalidStatus(status.to_string());
            assert!(error.to_string().contains("Invalid status"));
        }
    }

    #[test]
    fn invalid_role_with_various_values() {
        let roles = ["", "superadmin", "ADMIN", "root"];

        for role in roles {
            let error = UserError::InvalidRole(role.to_string());
            assert!(error.to_string().contains("Invalid role"));
        }
    }

    #[test]
    fn validation_with_various_messages() {
        let messages = [
            "",
            "Field is required",
            "Value must be between 1 and 100",
            "特殊字符",
        ];

        for msg in messages {
            let error = UserError::Validation(msg.to_string());
            let display = error.to_string();
            assert!(display.contains("Validation error"));
        }
    }
}

// ============================================================================
// Error Trait Implementation Tests
// ============================================================================

mod error_trait_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn user_error_is_error() {
        let error = UserError::Validation("test".to_string());
        // This test verifies that UserError implements std::error::Error
        let _: &dyn Error = &error;
    }

    #[test]
    fn not_found_source_is_none() {
        let error = UserError::NotFound(UserId::new("user-123".to_string()));
        assert!(error.source().is_none());
    }

    #[test]
    fn email_already_exists_source_is_none() {
        let error = UserError::EmailAlreadyExists("test@example.com".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn invalid_status_source_is_none() {
        let error = UserError::InvalidStatus("bad".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn invalid_role_source_is_none() {
        let error = UserError::InvalidRole("bad".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn validation_source_is_none() {
        let error = UserError::Validation("error".to_string());
        assert!(error.source().is_none());
    }
}

// ============================================================================
// Result Type Alias Tests
// ============================================================================

mod result_type_tests {
    use super::*;
    use systemprompt_users::Result;

    #[test]
    fn result_ok_variant() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn result_err_variant() {
        let result: Result<i32> = Err(UserError::Validation("test".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn result_with_string_value() {
        let result: Result<String> = Ok("success".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn result_with_option_value() {
        let result: Result<Option<String>> = Ok(Some("value".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn result_with_vec_value() {
        let result: Result<Vec<i32>> = Ok(vec![1, 2, 3]);
        assert!(result.is_ok());
    }

    #[test]
    fn result_err_can_be_matched() {
        let result: Result<()> = Err(UserError::NotFound(UserId::new("user-123".to_string())));

        match result {
            Ok(_) => panic!("Expected error"),
            Err(UserError::NotFound(id)) => {
                assert_eq!(id.to_string(), "user-123");
            }
            Err(_) => panic!("Expected NotFound error"),
        }
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn empty_user_id_in_not_found() {
        let error = UserError::NotFound(UserId::new("".to_string()));
        let display = error.to_string();
        assert!(display.contains("User not found"));
    }

    #[test]
    fn unicode_in_error_messages() {
        let error = UserError::Validation("名前は必須です".to_string());
        let display = error.to_string();
        assert!(display.contains("名前は必須です"));
    }

    #[test]
    fn special_characters_in_email() {
        let error = UserError::EmailAlreadyExists("user+special@example.com".to_string());
        let display = error.to_string();
        assert!(display.contains("user+special@example.com"));
    }

    #[test]
    fn long_validation_message() {
        let long_message = "x".repeat(10000);
        let error = UserError::Validation(long_message.clone());
        let display = error.to_string();
        assert!(display.contains(&long_message));
    }

    #[test]
    fn newlines_in_validation_message() {
        let error = UserError::Validation("Line 1\nLine 2\nLine 3".to_string());
        let display = error.to_string();
        assert!(display.contains("Line 1\nLine 2\nLine 3"));
    }
}
