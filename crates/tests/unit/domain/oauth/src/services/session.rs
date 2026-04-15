//! Tests for OAuth session management types and client credential validation

use std::sync::Arc;

use async_trait::async_trait;
use http::HeaderMap;

use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_oauth::services::session::AuthenticatedSessionInfo;
use systemprompt_oauth::{
    AnonymousSessionInfo, CreateAnonymousSessionInput, SessionCreationError,
    SessionCreationService,
};
use systemprompt_oauth::services::{generate_client_secret, hash_client_secret, verify_client_secret};
use systemprompt_traits::{
    AnalyticsProvider, AnalyticsResult, AnalyticsSession, AuthResult, AuthUser,
    CreateSessionInput, FingerprintProvider, SessionAnalytics, UserEvent, UserEventPublisher,
    UserProvider,
};

const TEST_CLIENT_SECRET: &str = "secret_TestClientSecretValue12345";
const TEST_SESSION_ID: &str = "sess_test-session-001";
const TEST_USER_ID: &str = "user_test-user-001";
const TEST_JWT_TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.test.signature";
const TEST_FINGERPRINT_HASH: &str = "fp_abc123def456";

struct MockAnalyticsProvider;

#[async_trait]
impl AnalyticsProvider for MockAnalyticsProvider {
    fn extract_analytics(
        &self,
        _headers: &HeaderMap,
        _uri: Option<&http::Uri>,
    ) -> SessionAnalytics {
        SessionAnalytics::default()
    }

    async fn create_session(&self, _input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        Ok(())
    }

    async fn find_recent_session_by_fingerprint(
        &self,
        _fingerprint: &str,
        _max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        Ok(None)
    }

    async fn find_session_by_id(
        &self,
        _session_id: &SessionId,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        Ok(None)
    }

    async fn migrate_user_sessions(
        &self,
        _from_user_id: &UserId,
        _to_user_id: &UserId,
    ) -> AnalyticsResult<u64> {
        Ok(0)
    }

    async fn mark_session_converted(&self, _session_id: &SessionId) -> AnalyticsResult<()> {
        Ok(())
    }
}

struct MockUserProvider;

#[async_trait]
impl UserProvider for MockUserProvider {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn find_by_email(&self, _email: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn find_by_name(&self, _name: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn create_user(
        &self,
        _name: &str,
        _email: &str,
        _full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new("user_new"),
            name: "newuser".to_string(),
            email: "new@example.com".to_string(),
            roles: vec![],
            is_active: true,
        })
    }

    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new("user_anon"),
            name: "anonymous".to_string(),
            email: "anon@example.com".to_string(),
            roles: vec![],
            is_active: true,
        })
    }

    async fn assign_roles(&self, _user_id: &UserId, _roles: &[String]) -> AuthResult<()> {
        Ok(())
    }
}

struct MockEventPublisher {
    events: std::sync::Mutex<Vec<String>>,
}

impl MockEventPublisher {
    fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl UserEventPublisher for MockEventPublisher {
    fn publish_user_event(&self, event: UserEvent) {
        self.events
            .lock()
            .unwrap()
            .push(format!("{:?}", event));
    }
}

struct MockFingerprintProvider;

#[async_trait]
impl FingerprintProvider for MockFingerprintProvider {
    async fn count_active_sessions(&self, _fingerprint: &str) -> AnalyticsResult<i64> {
        Ok(0)
    }

    async fn find_reusable_session(&self, _fingerprint: &str) -> AnalyticsResult<Option<String>> {
        Ok(None)
    }

    async fn upsert_fingerprint(
        &self,
        _fingerprint: &str,
        _ip_address: Option<&str>,
        _user_agent: Option<&str>,
        _screen_info: Option<&str>,
    ) -> AnalyticsResult<()> {
        Ok(())
    }
}

fn create_test_anonymous_session_info() -> AnonymousSessionInfo {
    AnonymousSessionInfo {
        session_id: SessionId::new(TEST_SESSION_ID.to_string()),
        user_id: UserId::new(TEST_USER_ID.to_string()),
        is_new: true,
        jwt_token: TEST_JWT_TOKEN.to_string(),
        fingerprint_hash: TEST_FINGERPRINT_HASH.to_string(),
    }
}

// ============================================================================
// AnonymousSessionInfo Tests
// ============================================================================

#[test]
fn test_anonymous_session_info_fields() {
    let info = create_test_anonymous_session_info();

    assert_eq!(info.session_id.as_str(), TEST_SESSION_ID);
    assert_eq!(info.user_id.as_str(), TEST_USER_ID);
    assert!(info.is_new);
    assert_eq!(info.jwt_token, TEST_JWT_TOKEN);
    assert_eq!(info.fingerprint_hash, TEST_FINGERPRINT_HASH);
}

#[test]
fn test_anonymous_session_info_clone() {
    let original = create_test_anonymous_session_info();
    let cloned = original.clone();

    assert_eq!(cloned.session_id.as_str(), original.session_id.as_str());
    assert_eq!(cloned.user_id.as_str(), original.user_id.as_str());
    assert_eq!(cloned.is_new, original.is_new);
    assert_eq!(cloned.jwt_token, original.jwt_token);
    assert_eq!(cloned.fingerprint_hash, original.fingerprint_hash);
}

#[test]
fn test_anonymous_session_info_debug() {
    let info = create_test_anonymous_session_info();
    let debug_output = format!("{:?}", info);

    assert!(debug_output.contains("AnonymousSessionInfo"));
    assert!(debug_output.contains(TEST_SESSION_ID));
    assert!(debug_output.contains(TEST_USER_ID));
}

#[test]
fn test_authenticated_session_info_fields() {
    let session_id = SessionId::new("sess_auth-session-001".to_string());
    let info = AuthenticatedSessionInfo {
        session_id: session_id.clone(),
    };

    assert_eq!(info.session_id.as_str(), "sess_auth-session-001");
}

// ============================================================================
// SessionCreationError Tests
// ============================================================================

#[test]
fn test_session_creation_error_user_not_found() {
    let error = SessionCreationError::UserNotFound {
        user_id: UserId::new(TEST_USER_ID),
    };
    let message = error.to_string();

    assert!(message.contains(TEST_USER_ID));
    assert!(message.contains("User not found"));
}

#[test]
fn test_session_creation_error_internal() {
    let error = SessionCreationError::Internal("database connection lost".to_string());
    let message = error.to_string();

    assert!(message.contains("database connection lost"));
}

#[test]
fn test_session_creation_error_display() {
    let user_error = SessionCreationError::UserNotFound {
        user_id: UserId::new("user_missing"),
    };
    let internal_error = SessionCreationError::Internal("timeout".to_string());

    let user_display = format!("{}", user_error);
    let internal_display = format!("{}", internal_error);

    assert!(!user_display.is_empty());
    assert!(!internal_display.is_empty());
    assert_ne!(user_display, internal_display);
}

#[test]
fn test_session_creation_error_is_std_error() {
    let error = SessionCreationError::Internal("test".to_string());
    let std_error: &dyn std::error::Error = &error;

    assert!(!std_error.to_string().is_empty());
}

// ============================================================================
// CreateAnonymousSessionInput Tests
// ============================================================================

#[test]
fn test_create_anonymous_session_input_debug() {
    let headers = HeaderMap::new();
    let client_id = ClientId::new("client_test123".to_string());
    let input = CreateAnonymousSessionInput {
        headers: &headers,
        uri: None,
        client_id: &client_id,
        jwt_secret: "test_secret",
        session_source: SessionSource::Web,
    };
    let debug_output = format!("{:?}", input);

    assert!(debug_output.contains("CreateAnonymousSessionInput"));
}

// ============================================================================
// SessionCreationService Construction Tests
// ============================================================================

#[test]
fn test_session_creation_service_new() {
    let analytics: Arc<dyn AnalyticsProvider> = Arc::new(MockAnalyticsProvider);
    let user: Arc<dyn UserProvider> = Arc::new(MockUserProvider);

    let service = SessionCreationService::new(analytics, user);
    let debug_output = format!("{:?}", service);

    assert!(debug_output.contains("SessionCreationService"));
    assert!(debug_output.contains("<provider>"));
}

#[test]
fn test_session_creation_service_with_event_publisher() {
    let analytics: Arc<dyn AnalyticsProvider> = Arc::new(MockAnalyticsProvider);
    let user: Arc<dyn UserProvider> = Arc::new(MockUserProvider);
    let publisher: Arc<dyn UserEventPublisher> = Arc::new(MockEventPublisher::new());

    let service = SessionCreationService::new(analytics, user).with_event_publisher(publisher);
    let debug_output = format!("{:?}", service);

    assert!(debug_output.contains("<publisher>"));
}

#[test]
fn test_session_creation_service_with_fingerprint_provider() {
    let analytics: Arc<dyn AnalyticsProvider> = Arc::new(MockAnalyticsProvider);
    let user: Arc<dyn UserProvider> = Arc::new(MockUserProvider);
    let fingerprint: Arc<dyn FingerprintProvider> = Arc::new(MockFingerprintProvider);

    let _service =
        SessionCreationService::new(analytics, user).with_fingerprint_provider(fingerprint);
}

#[test]
fn test_session_creation_service_debug() {
    let analytics: Arc<dyn AnalyticsProvider> = Arc::new(MockAnalyticsProvider);
    let user: Arc<dyn UserProvider> = Arc::new(MockUserProvider);
    let publisher: Arc<dyn UserEventPublisher> = Arc::new(MockEventPublisher::new());

    let without_publisher = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::clone(&user) as Arc<dyn UserProvider>,
    );
    let with_publisher = SessionCreationService::new(analytics, user)
        .with_event_publisher(publisher);

    let debug_without = format!("{:?}", without_publisher);
    let debug_with = format!("{:?}", with_publisher);

    assert!(debug_without.contains("None"));
    assert!(debug_with.contains("Some"));
}

// ============================================================================
// Client Secret Verification Tests
// ============================================================================

#[test]
fn test_verify_client_secret_correct() {
    let hash = hash_client_secret(TEST_CLIENT_SECRET).unwrap();
    let result = verify_client_secret(TEST_CLIENT_SECRET, &hash).unwrap();

    assert!(result);
}

#[test]
fn test_verify_client_secret_incorrect() {
    let hash = hash_client_secret(TEST_CLIENT_SECRET).unwrap();
    let result = verify_client_secret("wrong_secret_value", &hash).unwrap();

    assert!(!result);
}

#[test]
fn test_verify_client_secret_empty_secret() {
    let hash = hash_client_secret("some_secret").unwrap();
    let result = verify_client_secret("", &hash).unwrap();

    assert!(!result);
}

#[test]
fn test_verify_client_secret_empty_hash() {
    let result = verify_client_secret("some_secret", "");

    assert!(result.is_err());
}

#[test]
fn test_hash_client_secret_produces_bcrypt() {
    let hash = hash_client_secret(TEST_CLIENT_SECRET).unwrap();

    assert!(hash.starts_with("$2"));
}

#[test]
fn test_hash_client_secret_different_salts() {
    let hash1 = hash_client_secret(TEST_CLIENT_SECRET).unwrap();
    let hash2 = hash_client_secret(TEST_CLIENT_SECRET).unwrap();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_hash_then_verify_roundtrip() {
    let secrets = ["short", "a_longer_secret_value_here", "special!@#$%^&*()"];

    for secret in secrets {
        let hash = hash_client_secret(secret).unwrap();
        let verified = verify_client_secret(secret, &hash).unwrap();
        assert!(verified, "Roundtrip failed for secret: {}", secret);
    }
}

#[test]
fn test_generate_client_secret_format() {
    let secret = generate_client_secret();

    assert!(secret.starts_with("secret_"));
    assert_eq!(secret.len(), 7 + 64);
}
