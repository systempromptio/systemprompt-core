// OAuthState construction, optional-provider wiring, and Debug redaction.

use std::sync::Arc;

use async_trait::async_trait;
use http::HeaderMap;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{
    AnalyticsProvider, AnalyticsResult, AnalyticsSession, AuthResult, AuthUser, CreateSessionInput,
    ExtractSignals, FingerprintProvider, McpRegistryProvider, SessionAnalytics, UserEvent,
    UserEventPublisher, UserProvider,
};

struct NullAnalytics;

#[async_trait]
impl AnalyticsProvider for NullAnalytics {
    fn extract_analytics(
        &self,
        _headers: &HeaderMap,
        _signals: ExtractSignals<'_>,
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
    async fn find_active_session_by_id(
        &self,
        _session_id: &SessionId,
    ) -> AnalyticsResult<Option<systemprompt_traits::ActiveSession>> {
        Ok(None)
    }
    async fn revoke_session(&self, _session_id: &SessionId) -> AnalyticsResult<()> {
        Ok(())
    }
    async fn revoke_all_sessions_for_user(&self, _user_id: &UserId) -> AnalyticsResult<u64> {
        Ok(0)
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

struct NullUsers;

#[async_trait]
impl UserProvider for NullUsers {
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
        name: &str,
        email: &str,
        _full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new("user_state_test"),
            name: name.to_owned(),
            email: email.to_owned(),
            roles: vec![],
            is_active: true,
        })
    }
    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new("user_state_anon"),
            name: "anon".to_owned(),
            email: String::new(),
            roles: vec![],
            is_active: true,
        })
    }
    async fn assign_roles(&self, _user_id: &UserId, _roles: &[String]) -> AuthResult<()> {
        Ok(())
    }
    async fn find_or_create_federated(
        &self,
        _issuer: &str,
        _external_sub: &str,
        _claims: &systemprompt_traits::FederatedIdentityClaims,
    ) -> AuthResult<UserId> {
        Ok(UserId::new("user_state_fed"))
    }
}

struct NullFingerprints;

#[async_trait]
impl FingerprintProvider for NullFingerprints {
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

struct NullPublisher;

impl UserEventPublisher for NullPublisher {
    fn publish_user_event(&self, _event: UserEvent) {}
}

struct NullRegistry;

#[async_trait]
impl McpRegistryProvider for NullRegistry {
    async fn get_server(
        &self,
        name: &str,
    ) -> Result<systemprompt_traits::McpServerInfo, systemprompt_traits::RegistryError> {
        Err(systemprompt_traits::RegistryError::NotFound(
            name.to_owned(),
        ))
    }
    async fn list_enabled_servers(
        &self,
    ) -> Result<Vec<systemprompt_traits::McpServerInfo>, systemprompt_traits::RegistryError> {
        Ok(Vec::new())
    }
}

async fn base_state() -> Option<OAuthState> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(OAuthState::new(
        pool,
        Arc::new(NullAnalytics),
        Arc::new(NullUsers),
    ))
}

#[tokio::test]
async fn new_state_has_no_optional_providers() {
    let Some(state) = base_state().await else {
        return;
    };

    assert!(state.fingerprint_provider().is_none());
    assert!(state.event_publisher().is_none());
    assert!(state.mcp_registry().is_none());
    assert!(state.link_states().try_lock().expect("unlocked").is_empty());

    let debug = format!("{state:?}");
    assert!(debug.contains("OAuthState"));
    assert!(debug.contains("fingerprint_provider: None"));
}

#[tokio::test]
async fn builder_methods_attach_optional_providers() {
    let Some(state) = base_state().await else {
        return;
    };

    let state = state
        .with_fingerprint_provider(Arc::new(NullFingerprints))
        .with_event_publisher(Arc::new(NullPublisher))
        .with_mcp_registry(Arc::new(NullRegistry));

    assert!(state.fingerprint_provider().is_some());
    assert!(state.event_publisher().is_some());
    assert!(state.mcp_registry().is_some());
    let _pool = state.db_pool();
    let _analytics = state.analytics_provider();
    let _users = state.user_provider();

    let debug = format!("{state:?}");
    assert!(debug.contains("Some(\"<provider>\")"));

    let cloned = state.clone();
    assert!(cloned.mcp_registry().is_some());
}
