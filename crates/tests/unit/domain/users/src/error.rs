//! Unit tests for user error types.
//!
//! Tests cover:
//! - UserError enum variants
//! - Error display messages
//! - Error conversions (From implementations)
//! - Result type alias

use systemprompt_test_fixtures::unique_user_id;
use systemprompt_users::UserError;

mod user_error_display_tests {
    use super::*;

    #[test]
    fn not_found_displays_user_id() {
        let user_id = unique_user_id("error");
        let error = UserError::NotFound(user_id.clone());

        let display = error.to_string();
        assert!(display.contains("user not found"));
        assert!(display.contains(user_id.as_str()));
    }

    #[test]
    fn email_already_exists_displays_email() {
        let error = UserError::EmailAlreadyExists {
            email: "test@example.com".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("user already exists"));
        assert!(display.contains("test@example.com"));
    }

    #[test]
    fn validation_error_displays_message() {
        let error = UserError::Validation("Name cannot be empty".to_string());

        let display = error.to_string();
        assert!(display.contains("validation"));
        assert!(display.contains("Name cannot be empty"));
    }
}

mod user_error_construction_tests {
    use super::*;

    #[test]
    fn not_found_with_different_user_ids() {
        let id1 = unique_user_id("user-1");
        let id2 = unique_user_id("user-2");

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
            let error = UserError::EmailAlreadyExists {
                email: email.to_string(),
            };
            assert!(error.to_string().contains(email));
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
            assert!(display.contains("validation"));
        }
    }
}

mod error_trait_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn not_found_source_is_none() {
        let error = UserError::NotFound(unique_user_id("error"));
        assert!(error.source().is_none());
    }

    #[test]
    fn email_already_exists_source_is_none() {
        let error = UserError::EmailAlreadyExists {
            email: "test@example.com".to_string(),
        };
        assert!(error.source().is_none());
    }

    #[test]
    fn validation_source_is_none() {
        let error = UserError::Validation("error".to_string());
        assert!(error.source().is_none());
    }
}

mod result_type_tests {
    use super::*;
    use systemprompt_users::Result;

    #[test]
    fn result_ok_variant() {
        let result: Result<i32> = Ok(42);
        let val = result.expect("expected success");
        assert_eq!(val, 42);
    }

    #[test]
    fn result_err_variant() {
        let result: Result<i32> = Err(UserError::Validation("test".to_string()));
        result.unwrap_err();
    }

    #[test]
    fn result_with_string_value() {
        let result: Result<String> = Ok("success".to_string());
        result.expect("expected success");
    }

    #[test]
    fn result_with_option_value() {
        let result: Result<Option<String>> = Ok(Some("value".to_string()));
        result.expect("expected success");
    }

    #[test]
    fn result_with_vec_value() {
        let result: Result<Vec<i32>> = Ok(vec![1, 2, 3]);
        result.expect("expected success");
    }

    #[test]
    fn result_err_can_be_matched() {
        let user_id = unique_user_id("error");
        let result: Result<()> = Err(UserError::NotFound(user_id.clone()));

        match result {
            Ok(_) => panic!("Expected error"),
            Err(UserError::NotFound(id)) => {
                assert_eq!(id, user_id);
            },
            Err(_) => panic!("Expected NotFound error"),
        }
    }
}

mod edge_case_tests {
    use super::*;

    #[test]
    fn empty_user_id_in_not_found() {
        let error = UserError::NotFound(unique_user_id("error"));
        let display = error.to_string();
        assert!(display.contains("user not found"));
    }

    #[test]
    fn unicode_in_error_messages() {
        let error = UserError::Validation("名前は必須です".to_string());
        let display = error.to_string();
        assert!(display.contains("名前は必須です"));
    }

    #[test]
    fn special_characters_in_email() {
        let error = UserError::EmailAlreadyExists {
            email: "user+special@example.com".to_string(),
        };
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
