//! Tests for UserExport and ImportResult

mod user_export_tests {
    use chrono::Utc;
    use systemprompt_sync::database::UserExport;

    #[test]
    fn creation() {
        let now = Utc::now();
        let user = UserExport {
            id: "user_123".to_string(),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            display_name: Some("Test".to_string()),
            status: "active".to_string(),
            email_verified: true,
            roles: vec!["user".to_string(), "admin".to_string()],
            is_bot: false,
            is_scanner: false,
            avatar_url: Some("https://example.com/avatar.png".to_string()),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(user.id, "user_123");
        assert!(user.email_verified);
        assert_eq!(user.roles.len(), 2);
    }

    #[test]
    fn minimal() {
        let now = Utc::now();
        let user = UserExport {
            id: "user_min".to_string(),
            name: "minimal".to_string(),
            email: "min@example.com".to_string(),
            full_name: None,
            display_name: None,
            status: "pending".to_string(),
            email_verified: false,
            roles: vec![],
            is_bot: true,
            is_scanner: true,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        };

        assert!(user.full_name.is_none());
        assert!(user.roles.is_empty());
        assert!(user.is_bot);
    }

    #[test]
    fn serialization() {
        let now = Utc::now();
        let user = UserExport {
            id: "user_ser".to_string(),
            name: "seruser".to_string(),
            email: "ser@example.com".to_string(),
            full_name: None,
            display_name: None,
            status: "active".to_string(),
            email_verified: true,
            roles: vec!["user".to_string()],
            is_bot: false,
            is_scanner: false,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&user).expect("serialize user export");
        assert!(json.contains("\"id\":\"user_ser\""));
        assert!(json.contains("\"email_verified\":true"));
    }
}

mod import_result_tests {
    use systemprompt_sync::database::ImportResult;

    #[test]
    fn creation() {
        let result = ImportResult {
            created: 10,
            updated: 5,
            skipped: 2,
        };

        assert_eq!(result.created, 10);
        assert_eq!(result.updated, 5);
        assert_eq!(result.skipped, 2);
    }

    #[test]
    fn zero_values() {
        let result = ImportResult {
            created: 0,
            updated: 0,
            skipped: 0,
        };

        assert_eq!(result.created, 0);
    }

    #[test]
    fn serialization() {
        let result = ImportResult {
            created: 100,
            updated: 50,
            skipped: 10,
        };

        let json = serde_json::to_string(&result).expect("serialize import result");
        assert!(json.contains("\"created\":100"));
    }

    #[test]
    fn copy() {
        let result = ImportResult {
            created: 1,
            updated: 2,
            skipped: 3,
        };

        let copied = result;
        assert_eq!(result.created, copied.created);
    }
}
