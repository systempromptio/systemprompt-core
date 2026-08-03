//! Tests for error module types and implementations.

use std::time::Duration;
use systemprompt_ai::error::{AiError, RepositoryError};
use systemprompt_database::resilience::Outcome;
use systemprompt_identifiers::McpServerId;
use uuid::Uuid;

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
            service_id: McpServerId::new("github-mcp"),
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
        let err = AiError::ConfigurationError {
            message: "missing api_key".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("missing api_key"));
        assert!(msg.contains("Configuration error"));
    }

    #[test]
    fn database_error_from_anyhow() {
        let err: AiError = AiError::DatabaseError {
            message: "connection refused".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("connection refused"));
    }

    #[test]
    fn mcp_service_not_found_error() {
        let err = AiError::McpServiceNotFound {
            service_id: McpServerId::new("custom-service"),
        };
        let msg = err.to_string();
        assert!(msg.contains("custom-service"));
        assert!(msg.contains("not found or not configured"));
    }

    #[test]
    fn mcp_authentication_missing_error() {
        let err = AiError::McpAuthenticationMissing {
            service_id: McpServerId::new("oauth-service"),
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
        let err = AiError::StorageError {
            message: "disk full".to_string(),
        };
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

mod classify_tests {
    use super::*;

    #[test]
    fn http_status_429_is_transient_with_retry_after() {
        let err = AiError::HttpStatus {
            provider: "anthropic".to_string(),
            status: 429,
            retry_after: Some(Duration::from_secs(30)),
            body: "slow down".to_string(),
        };
        assert!(matches!(
            err.classify(),
            Outcome::Transient {
                retry_after: Some(d)
            } if d == Duration::from_secs(30)
        ));
        assert!(err.to_string().contains("429"));
    }

    #[test]
    fn http_status_400_is_permanent() {
        let err = AiError::HttpStatus {
            provider: "openai".to_string(),
            status: 400,
            retry_after: None,
            body: "bad request".to_string(),
        };
        assert!(matches!(err.classify(), Outcome::Permanent));
    }

    #[test]
    fn http_status_503_is_transient() {
        let err = AiError::HttpStatus {
            provider: "gemini".to_string(),
            status: 503,
            retry_after: None,
            body: String::new(),
        };
        assert!(matches!(
            err.classify(),
            Outcome::Transient { retry_after: None }
        ));
    }

    #[test]
    fn rate_limit_is_transient() {
        let err = AiError::RateLimit {
            provider: "anthropic".to_string(),
            details: "tpm exceeded".to_string(),
        };
        assert!(matches!(
            err.classify(),
            Outcome::Transient { retry_after: None }
        ));
    }

    #[test]
    fn timeout_is_transient_and_displays_provider() {
        let err = AiError::Timeout {
            provider: "openai".to_string(),
            after_ms: 5000,
        };
        assert!(matches!(
            err.classify(),
            Outcome::Transient { retry_after: None }
        ));
        let msg = err.to_string();
        assert!(msg.contains("openai"));
        assert!(msg.contains("5000"));
    }

    #[test]
    fn circuit_open_is_permanent() {
        let err = AiError::CircuitOpen {
            provider: "openai".to_string(),
        };
        assert!(matches!(err.classify(), Outcome::Permanent));
        assert!(err.to_string().contains("Circuit breaker open"));
    }

    #[test]
    fn dependency_unavailable_displays_and_is_permanent() {
        let err = AiError::DependencyUnavailable {
            provider: "gemini".to_string(),
        };
        assert!(matches!(err.classify(), Outcome::Permanent));
        assert!(err.to_string().contains("concurrency limit"));
    }

    #[test]
    fn internal_error_displays_message() {
        let err = AiError::Internal("unexpected state".to_string());
        assert!(err.to_string().contains("unexpected state"));
        assert!(matches!(err.classify(), Outcome::Permanent));
    }

    #[test]
    fn io_error_from_std_io() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let err: AiError = io.into();
        assert!(err.to_string().contains("I/O error"));
        assert!(matches!(err.classify(), Outcome::Permanent));
    }
}

mod from_error_response_tests {
    use super::*;

    #[tokio::test]
    async fn captures_status_body_and_retry_after() {
        let http = http::Response::builder()
            .status(429)
            .header("retry-after", "7")
            .body("rate limited body")
            .expect("response");
        let err = AiError::from_error_response("openai", reqwest::Response::from(http)).await;

        match &err {
            AiError::HttpStatus {
                provider,
                status,
                retry_after,
                body,
            } => {
                assert_eq!(provider, "openai");
                assert_eq!(*status, 429);
                assert_eq!(*retry_after, Some(Duration::from_secs(7)));
                assert_eq!(body, "rate limited body");
            },
            other => panic!("expected HttpStatus, got {other:?}"),
        }
        assert!(matches!(
            err.classify(),
            Outcome::Transient {
                retry_after: Some(d)
            } if d == Duration::from_secs(7)
        ));
    }

    #[tokio::test]
    async fn non_numeric_retry_after_is_ignored() {
        let http = http::Response::builder()
            .status(503)
            .header("retry-after", "Wed, 21 Oct 2026 07:28:00 GMT")
            .body("")
            .expect("response");
        let err = AiError::from_error_response("gemini", reqwest::Response::from(http)).await;

        assert!(matches!(
            err,
            AiError::HttpStatus {
                status: 503,
                retry_after: None,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn client_error_without_retry_after_is_permanent() {
        let http = http::Response::builder()
            .status(401)
            .body("unauthorized")
            .expect("response");
        let err = AiError::from_error_response("anthropic", reqwest::Response::from(http)).await;

        assert!(matches!(err.classify(), Outcome::Permanent));
    }
}

// The reqwest-backed arms of `classify`. Everything above constructs the error
// by hand; these two require a real transport failure, and they matter because
// the resilience guard retries on `Transient` and gives up on `Permanent`.
mod transport_classification {
    use super::*;
    use systemprompt_ai::services::providers::http_client::build_client;

    #[tokio::test]
    async fn a_connection_failure_classifies_as_transient() {
        let client = build_client(Duration::from_millis(200), Duration::from_millis(50));

        // Port 1 on loopback refuses immediately: a connect error, not a status.
        let reqwest_err = client
            .get("http://127.0.0.1:1/never-listening")
            .send()
            .await
            .expect_err("nothing is listening on that port");
        assert!(
            reqwest_err.is_connect() || reqwest_err.is_timeout(),
            "the fixture must produce a transport failure, got {reqwest_err}"
        );

        let err = AiError::from(reqwest_err);
        assert!(
            matches!(err.classify(), Outcome::Transient { retry_after: None }),
            "an upstream we could not reach is worth retrying, and carries no \
             server-supplied backoff"
        );
    }

    #[tokio::test]
    async fn a_request_timeout_classifies_as_transient() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
            .mount(&server)
            .await;

        let client = build_client(Duration::from_millis(50), Duration::from_secs(1));
        let reqwest_err = client
            .get(format!("{}/slow", server.uri()))
            .send()
            .await
            .expect_err("the client's request timeout must fire first");
        assert!(
            reqwest_err.is_timeout(),
            "the fixture must produce a timeout, got {reqwest_err}"
        );

        let err = AiError::from(reqwest_err);
        assert!(
            matches!(err.classify(), Outcome::Transient { retry_after: None }),
            "a request that ran out of time is worth retrying"
        );
    }

    #[tokio::test]
    async fn a_transport_error_that_is_neither_connect_nor_timeout_is_permanent() {
        let client = build_client(Duration::from_secs(5), Duration::from_secs(1));

        // An unsupported scheme fails at request construction, not transport.
        let reqwest_err = client
            .get("nonsense://example.invalid/path")
            .send()
            .await
            .expect_err("an unsupported scheme cannot be sent");
        assert!(
            !reqwest_err.is_timeout() && !reqwest_err.is_connect(),
            "the fixture must not be a retryable transport failure, got {reqwest_err}"
        );

        let err = AiError::from(reqwest_err);
        assert!(
            matches!(err.classify(), Outcome::Permanent),
            "a malformed request will fail identically on every retry"
        );
    }

    #[tokio::test]
    async fn the_built_client_honours_the_request_timeout_it_was_given() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_delay(Duration::from_millis(400)),
            )
            .mount(&server)
            .await;

        let impatient = build_client(Duration::from_millis(50), Duration::from_secs(1));
        assert!(
            impatient.get(server.uri()).send().await.is_err(),
            "a 50ms budget must not tolerate a 400ms response"
        );

        let patient = build_client(Duration::from_secs(5), Duration::from_secs(1));
        let response = patient
            .get(server.uri())
            .send()
            .await
            .expect("a 5s budget must tolerate the same response");
        assert!(response.status().is_success());
    }
}
