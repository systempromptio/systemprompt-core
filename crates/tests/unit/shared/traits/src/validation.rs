//! Tests for validation module types.

use systemprompt_traits::{MetadataValidation, Validate, ValidationError, ValidationResult};

mod validation_error_tests {
    use super::*;

    #[test]
    fn new_creates_error_without_context() {
        let err = ValidationError::new("email", "Invalid email format");

        assert_eq!(err.field, "email");
        assert_eq!(err.message, "Invalid email format");
        assert!(err.context.is_none());
    }

    #[test]
    fn new_accepts_string_types() {
        let err = ValidationError::new(
            String::from("username"),
            String::from("Too short"),
        );

        assert_eq!(err.field, "username");
        assert_eq!(err.message, "Too short");
    }

    #[test]
    fn with_context_adds_context() {
        let err = ValidationError::new("password", "Too weak")
            .with_context("Must contain uppercase and numbers");

        assert!(err.context.is_some());
        assert_eq!(err.context.unwrap(), "Must contain uppercase and numbers");
    }

    #[test]
    fn with_context_is_chainable() {
        let err = ValidationError::new("field", "message")
            .with_context("context");

        assert_eq!(err.field, "field");
        assert_eq!(err.message, "message");
        assert_eq!(err.context, Some("context".to_string()));
    }

    #[test]
    fn display_without_context() {
        let err = ValidationError::new("name", "Cannot be empty");
        let display = format!("{}", err);

        assert!(display.contains("VALIDATION ERROR"));
        assert!(display.contains("[name]"));
        assert!(display.contains("Cannot be empty"));
        assert!(!display.contains("context"));
    }

    #[test]
    fn display_with_context() {
        let err = ValidationError::new("age", "Must be positive")
            .with_context("User registration form");
        let display = format!("{}", err);

        assert!(display.contains("VALIDATION ERROR"));
        assert!(display.contains("[age]"));
        assert!(display.contains("Must be positive"));
        assert!(display.contains("context: User registration form"));
    }

    #[test]
    fn validation_error_is_std_error() {
        let err = ValidationError::new("test", "error");
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn validation_error_is_clone() {
        let err = ValidationError::new("field", "message")
            .with_context("ctx");
        let cloned = err.clone();

        assert_eq!(err.field, cloned.field);
        assert_eq!(err.message, cloned.message);
        assert_eq!(err.context, cloned.context);
    }

    #[test]
    fn validation_error_is_debug() {
        let err = ValidationError::new("test", "debug test");
        let debug_str = format!("{:?}", err);

        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("debug test"));
    }
}

mod validate_trait_tests {
    use super::*;

    #[derive(Debug)]
    struct ValidData {
        name: String,
    }

    impl Validate for ValidData {
        fn validate(&self) -> ValidationResult<()> {
            if self.name.is_empty() {
                return Err(ValidationError::new("name", "Name cannot be empty"));
            }
            Ok(())
        }
    }

    #[test]
    fn validate_returns_ok_for_valid_data() {
        let data = ValidData {
            name: "John".to_string(),
        };
        assert!(data.validate().is_ok());
    }

    #[test]
    fn validate_returns_err_for_invalid_data() {
        let data = ValidData {
            name: String::new(),
        };
        let result = data.validate();
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.field, "name");
    }
}

mod metadata_validation_trait_tests {
    use super::*;

    #[derive(Debug)]
    struct Metadata {
        title: String,
        description: String,
        author: String,
    }

    impl Validate for Metadata {
        fn validate(&self) -> ValidationResult<()> {
            self.validate_required_fields()
        }
    }

    impl MetadataValidation for Metadata {
        fn required_string_fields(&self) -> Vec<(&'static str, &str)> {
            vec![
                ("title", &self.title),
                ("description", &self.description),
                ("author", &self.author),
            ]
        }
    }

    #[test]
    fn validate_required_fields_passes_when_all_present() {
        let meta = Metadata {
            title: "My Title".to_string(),
            description: "A description".to_string(),
            author: "Author Name".to_string(),
        };

        assert!(meta.validate_required_fields().is_ok());
    }

    #[test]
    fn validate_required_fields_fails_on_empty_title() {
        let meta = Metadata {
            title: String::new(),
            description: "Description".to_string(),
            author: "Author".to_string(),
        };

        let result = meta.validate_required_fields();
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.field, "title");
        assert!(err.message.contains("cannot be empty"));
    }

    #[test]
    fn validate_required_fields_fails_on_first_empty_field() {
        let meta = Metadata {
            title: "Title".to_string(),
            description: String::new(),
            author: String::new(),
        };

        let result = meta.validate_required_fields();
        assert!(result.is_err());

        let err = result.unwrap_err();
        // Should fail on description first since it comes before author
        assert_eq!(err.field, "description");
    }

    #[test]
    fn validate_uses_validate_required_fields() {
        let meta = Metadata {
            title: "Title".to_string(),
            description: "Desc".to_string(),
            author: String::new(),
        };

        let result = meta.validate();
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.field, "author");
    }
}
