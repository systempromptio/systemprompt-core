//! Tests for error module types and implementations.

use systemprompt_core_ai::error::{AiError, RepositoryError};
use uuid::Uuid;

// sqlx is needed for Database error variants
use sqlx;

mod ai_error_tests {
    use super::*;

    #[test]
    fn model_not_specified_error_displays_provider() {
        let err = AiError::ModelNotSpecified {
            provider: "anthropic".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("anthropic"));
        assert!(msg.contains("Model not specified"));
    }

    #[test]
    fn missing_metadata_error_displays_field() {
        let err = AiError::MissingMetadata {
            field: "user_id".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("user_id"));
        assert!(msg.contains("missing required field"));
    }

    #[test]
    fn missing_user_context_error() {
        let err = AiError::MissingUserContext;
        let msg = err.to_string();
        assert!(msg.contains("User context required"));
    }

    #[test]
    fn empty_provider_response_error() {
        let err = AiError::EmptyProviderResponse {
            provider: "openai".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("openai"));
        assert!(msg.contains("empty response"));
    }

    #[test]
    fn invalid_tool_schema_error() {
        let err = AiError::InvalidToolSchema {
            reason: "missing required field 'name'".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("missing required field 'name'"));
        assert!(msg.contains("schema validation failed"));
    }

    #[test]
    fn authentication_required_error() {
        let err = AiError::AuthenticationRequired {
            service_id: "github-mcp".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("github-mcp"));
        assert!(msg.contains("Authentication required"));
    }

    #[test]
    fn structured_output_failed_error() {
        let err = AiError::StructuredOutputFailed {
            retries: 3,
            details: "JSON schema mismatch".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("3"));
        assert!(msg.contains("JSON schema mismatch"));
    }

    #[test]
    fn provider_error_displays_message() {
        let err = AiError::ProviderError {
            provider: "gemini".to_string(),
            message: "rate limit exceeded".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("gemini"));
        assert!(msg.contains("rate limit exceeded"));
    }

    #[test]
    fn serialization_error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let err: AiError = json_err.into();
        let msg = err.to_string();
        assert!(msg.contains("Serialization failed"));
    }

    #[test]
    fn message_serialization_failed_error() {
        let err = AiError::MessageSerializationFailed;
        let msg = err.to_string();
        assert!(msg.contains("Message history cannot be serialized"));
    }

    #[test]
    fn missing_tool_field_error() {
        let err = AiError::MissingToolField {
            tool_name: "search".to_string(),
            field: "description".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("search"));
        assert!(msg.contains("description"));
    }

    #[test]
    fn empty_tool_description_error() {
        let err = AiError::EmptyToolDescription {
            tool_name: "calculator".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("calculator"));
        assert!(msg.contains("cannot be empty"));
    }

    #[test]
    fn no_tool_calls_error() {
        let err = AiError::NoToolCalls;
        let msg = err.to_string();
        assert!(msg.contains("No tool calls found"));
    }

    #[test]
    fn rate_limit_error() {
        let err = AiError::RateLimit {
            provider: "anthropic".to_string(),
            details: "retry after 60s".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("anthropic"));
        assert!(msg.contains("retry after 60s"));
        assert!(msg.contains("Rate limit"));
    }

    #[test]
    fn authentication_failed_error() {
        let err = AiError::AuthenticationFailed {
            provider: "openai".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("openai"));
        assert!(msg.contains("Invalid API credentials"));
    }

    #[test]
    fn configuration_error() {
        let err = AiError::ConfigurationError("missing api_key".to_string());
        let msg = err.to_string();
        assert!(msg.contains("missing api_key"));
        assert!(msg.contains("Configuration error"));
    }

    #[test]
    fn database_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("connection refused");
        let err: AiError = AiError::DatabaseError(anyhow_err);
        let msg = err.to_string();
        assert!(msg.contains("connection refused"));
    }

    #[test]
    fn mcp_service_not_found_error() {
        let err = AiError::McpServiceNotFound {
            service_id: "custom-service".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("custom-service"));
        assert!(msg.contains("not found or not configured"));
    }

    #[test]
    fn mcp_authentication_missing_error() {
        let err = AiError::McpAuthenticationMissing {
            service_id: "oauth-service".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("oauth-service"));
        assert!(msg.contains("OAuth authentication"));
    }

    #[test]
    fn service_auth_check_failed_error() {
        let err = AiError::ServiceAuthCheckFailed {
            details: "timeout".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn storage_error() {
        let err = AiError::StorageError("disk full".to_string());
        let msg = err.to_string();
        assert!(msg.contains("disk full"));
        assert!(msg.contains("Storage operation failed"));
    }

    #[test]
    fn invalid_input_error() {
        let err = AiError::InvalidInput("prompt cannot be empty".to_string());
        let msg = err.to_string();
        assert!(msg.contains("prompt cannot be empty"));
        assert!(msg.contains("Invalid input"));
    }

    #[test]
    fn with_context_adds_context_message() {
        let err = AiError::NoToolCalls;
        let anyhow_err = err.with_context("while processing agent request");
        let msg = anyhow_err.to_string();
        assert!(msg.contains("while processing agent request"));
        assert!(msg.contains("No tool calls found"));
    }

    #[test]
    fn repository_error_converts_to_ai_error() {
        let repo_err = RepositoryError::NotFound(Uuid::nil());
        let ai_err: AiError = repo_err.into();
        let msg = ai_err.to_string();
        assert!(msg.contains("Database operation failed"));
    }
}

mod repository_error_tests {
    use super::*;

    #[test]
    fn not_found_error_displays_uuid() {
        let uuid = Uuid::new_v4();
        let err = RepositoryError::NotFound(uuid);
        let msg = err.to_string();
        assert!(msg.contains(&uuid.to_string()));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn database_error_from_sqlx() {
        let err = RepositoryError::Database(sqlx::Error::RowNotFound);
        let msg = err.to_string();
        assert!(msg.contains("Database error"));
    }

    #[test]
    fn invalid_data_error() {
        let err = RepositoryError::InvalidData {
            field: "status".to_string(),
            reason: "unknown value 'unknown_status'".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("status"));
        assert!(msg.contains("unknown value"));
    }

    #[test]
    fn pool_initialization_error() {
        let err = RepositoryError::PoolInitialization("connection timeout".to_string());
        let msg = err.to_string();
        assert!(msg.contains("connection timeout"));
    }
}
