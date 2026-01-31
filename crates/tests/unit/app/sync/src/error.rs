//! Tests for SyncError

use systemprompt_sync::SyncError;

mod error_variants_tests {
    use super::*;

    #[test]
    fn api_error_formats_with_status_and_message() {
        let error = SyncError::ApiError {
            status: 404,
            message: "Not found".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("404"));
        assert!(msg.contains("Not found"));
    }

    #[test]
    fn unauthorized_message() {
        let error = SyncError::Unauthorized;
        let msg = error.to_string();
        assert!(msg.to_lowercase().contains("unauthorized"));
    }

    #[test]
    fn tenant_no_app_message() {
        let error = SyncError::TenantNoApp;
        let msg = error.to_string();
        assert!(msg.contains("app"));
    }

    #[test]
    fn not_project_root_message() {
        let error = SyncError::NotProjectRoot;
        let msg = error.to_string();
        assert!(msg.contains("root") || msg.contains("infrastructure"));
    }

    #[test]
    fn command_failed_contains_command() {
        let error = SyncError::CommandFailed {
            command: "docker build".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("docker build"));
    }

    #[test]
    fn docker_login_failed_message() {
        let error = SyncError::DockerLoginFailed;
        let msg = error.to_string();
        assert!(msg.to_lowercase().contains("docker"));
    }

    #[test]
    fn git_sha_unavailable_message() {
        let error = SyncError::GitShaUnavailable;
        let msg = error.to_string();
        assert!(msg.to_lowercase().contains("git") || msg.to_lowercase().contains("sha"));
    }

    #[test]
    fn missing_config_contains_message() {
        let error = SyncError::MissingConfig("database_url".to_string());
        let msg = error.to_string();
        assert!(msg.contains("database_url"));
    }
}

mod is_retryable_tests {
    use super::*;

    #[test]
    fn api_error_502_is_retryable() {
        let error = SyncError::ApiError {
            status: 502,
            message: "Bad gateway".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn api_error_503_is_retryable() {
        let error = SyncError::ApiError {
            status: 503,
            message: "Service unavailable".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn api_error_504_is_retryable() {
        let error = SyncError::ApiError {
            status: 504,
            message: "Gateway timeout".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn api_error_429_is_retryable() {
        let error = SyncError::ApiError {
            status: 429,
            message: "Too many requests".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn api_error_400_is_not_retryable() {
        let error = SyncError::ApiError {
            status: 400,
            message: "Bad request".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn api_error_401_is_not_retryable() {
        let error = SyncError::ApiError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn api_error_404_is_not_retryable() {
        let error = SyncError::ApiError {
            status: 404,
            message: "Not found".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn api_error_500_is_not_retryable() {
        let error = SyncError::ApiError {
            status: 500,
            message: "Internal server error".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn unauthorized_is_not_retryable() {
        let error = SyncError::Unauthorized;
        assert!(!error.is_retryable());
    }

    #[test]
    fn tenant_no_app_is_not_retryable() {
        let error = SyncError::TenantNoApp;
        assert!(!error.is_retryable());
    }

    #[test]
    fn not_project_root_is_not_retryable() {
        let error = SyncError::NotProjectRoot;
        assert!(!error.is_retryable());
    }

    #[test]
    fn command_failed_is_not_retryable() {
        let error = SyncError::CommandFailed {
            command: "cmd".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn docker_login_failed_is_not_retryable() {
        let error = SyncError::DockerLoginFailed;
        assert!(!error.is_retryable());
    }

    #[test]
    fn git_sha_unavailable_is_not_retryable() {
        let error = SyncError::GitShaUnavailable;
        assert!(!error.is_retryable());
    }

    #[test]
    fn missing_config_is_not_retryable() {
        let error = SyncError::MissingConfig("cfg".to_string());
        assert!(!error.is_retryable());
    }
}

mod error_traits_tests {
    use super::*;

    #[test]
    fn error_is_debug() {
        let error = SyncError::Unauthorized;
        let debug = format!("{:?}", error);
        assert!(debug.contains("Unauthorized"));
    }

    #[test]
    fn error_implements_std_error() {
        let error: Box<dyn std::error::Error> = Box::new(SyncError::Unauthorized);
        assert!(!error.to_string().is_empty());
    }
}
