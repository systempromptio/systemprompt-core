//! Tests for auth module types and implementations.

use systemprompt_traits::{
    AuthAction, AuthPermission, AuthProviderError, TokenClaims, TokenPair,
};

mod token_pair_tests {
    use super::*;

    #[test]
    fn new_creates_bearer_token_type() {
        let pair = TokenPair::new(
            "access123".to_string(),
            Some("refresh456".to_string()),
            3600,
        );

        assert_eq!(pair.access_token, "access123");
        assert_eq!(pair.refresh_token, Some("refresh456".to_string()));
        assert_eq!(pair.expires_in, 3600);
        assert_eq!(pair.token_type, "Bearer");
    }

    #[test]
    fn new_with_no_refresh_token() {
        let pair = TokenPair::new("access123".to_string(), None, 7200);

        assert_eq!(pair.access_token, "access123");
        assert!(pair.refresh_token.is_none());
        assert_eq!(pair.expires_in, 7200);
        assert_eq!(pair.token_type, "Bearer");
    }

    #[test]
    fn clone_produces_equal_token_pair() {
        let pair = TokenPair::new(
            "access".to_string(),
            Some("refresh".to_string()),
            1800,
        );
        let cloned = pair.clone();

        assert_eq!(pair.access_token, cloned.access_token);
        assert_eq!(pair.refresh_token, cloned.refresh_token);
        assert_eq!(pair.expires_in, cloned.expires_in);
        assert_eq!(pair.token_type, cloned.token_type);
    }
}

mod token_claims_tests {
    use super::*;

    #[test]
    fn can_create_token_claims() {
        let claims = TokenClaims {
            subject: "user123".to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            audiences: vec!["api".to_string(), "web".to_string()],
            permissions: vec!["read".to_string(), "write".to_string()],
            expires_at: 1700000000,
            issued_at: 1699996400,
        };

        assert_eq!(claims.subject, "user123");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
        assert_eq!(claims.audiences.len(), 2);
        assert_eq!(claims.permissions.len(), 2);
    }

    #[test]
    fn token_claims_without_email() {
        let claims = TokenClaims {
            subject: "user456".to_string(),
            username: "anotheruser".to_string(),
            email: None,
            audiences: vec![],
            permissions: vec![],
            expires_at: 0,
            issued_at: 0,
        };

        assert!(claims.email.is_none());
        assert!(claims.audiences.is_empty());
        assert!(claims.permissions.is_empty());
    }
}

mod auth_action_tests {
    use super::*;

    #[test]
    fn as_str_returns_correct_values() {
        assert_eq!(AuthAction::Read.as_str(), "read");
        assert_eq!(AuthAction::Write.as_str(), "write");
        assert_eq!(AuthAction::Delete.as_str(), "delete");
        assert_eq!(AuthAction::Admin.as_str(), "admin");
    }

    #[test]
    fn custom_action_returns_custom_string() {
        let custom = AuthAction::Custom("execute".to_string());
        assert_eq!(custom.as_str(), "execute");

        let another = AuthAction::Custom("approve".to_string());
        assert_eq!(another.as_str(), "approve");
    }

    #[test]
    fn auth_actions_are_equal() {
        assert_eq!(AuthAction::Read, AuthAction::Read);
        assert_eq!(AuthAction::Write, AuthAction::Write);
        assert_eq!(
            AuthAction::Custom("test".to_string()),
            AuthAction::Custom("test".to_string())
        );
    }

    #[test]
    fn auth_actions_are_not_equal() {
        assert_ne!(AuthAction::Read, AuthAction::Write);
        assert_ne!(
            AuthAction::Custom("a".to_string()),
            AuthAction::Custom("b".to_string())
        );
    }
}

mod auth_permission_tests {
    use super::*;

    #[test]
    fn new_creates_permission() {
        let perm = AuthPermission::new("documents", AuthAction::Read);

        assert_eq!(perm.resource, "documents");
        assert_eq!(perm.action, AuthAction::Read);
    }

    #[test]
    fn new_accepts_string() {
        let perm = AuthPermission::new(String::from("users"), AuthAction::Write);

        assert_eq!(perm.resource, "users");
        assert_eq!(perm.action, AuthAction::Write);
    }

    #[test]
    fn new_with_custom_action() {
        let perm = AuthPermission::new("tasks", AuthAction::Custom("assign".to_string()));

        assert_eq!(perm.resource, "tasks");
        assert_eq!(perm.action, AuthAction::Custom("assign".to_string()));
    }

    #[test]
    fn permissions_equality() {
        let perm1 = AuthPermission::new("resource", AuthAction::Read);
        let perm2 = AuthPermission::new("resource", AuthAction::Read);
        let perm3 = AuthPermission::new("resource", AuthAction::Write);

        assert_eq!(perm1, perm2);
        assert_ne!(perm1, perm3);
    }
}

mod auth_provider_error_tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        assert_eq!(
            AuthProviderError::InvalidCredentials.to_string(),
            "Invalid credentials"
        );
        assert_eq!(
            AuthProviderError::UserNotFound.to_string(),
            "User not found"
        );
        assert_eq!(
            AuthProviderError::InvalidToken.to_string(),
            "Invalid token"
        );
        assert_eq!(
            AuthProviderError::TokenExpired.to_string(),
            "Token expired"
        );
        assert_eq!(
            AuthProviderError::InsufficientPermissions.to_string(),
            "Insufficient permissions"
        );
    }

    #[test]
    fn internal_error_includes_message() {
        let err = AuthProviderError::Internal("Database connection failed".to_string());
        assert_eq!(err.to_string(), "Internal error: Database connection failed");
    }

    #[test]
    fn from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("Something went wrong");
        let auth_err: AuthProviderError = anyhow_err.into();

        match auth_err {
            AuthProviderError::Internal(msg) => {
                assert!(msg.contains("Something went wrong"));
            }
            _ => panic!("Expected Internal error variant"),
        }
    }
}
