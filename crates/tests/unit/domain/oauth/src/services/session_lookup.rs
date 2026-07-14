//! Tests for `SessionCreationService` session-establishment flows: reuse at
//! the fingerprint session limit, lookup of an existing recent session, fresh
//! anonymous-session creation, authenticated-session creation, and the
//! anonymous-user resolution used by the session middleware. Providers are
//! configurable in-memory mocks; JWTs are minted against the fixture signing
//! key.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use http::HeaderMap;

use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_oauth::{
    CreateAnonymousSessionInput, SessionCreationError, SessionCreationService,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, install_test_signing_key};
use systemprompt_traits::{
    AnalyticsProvider, AnalyticsResult, AnalyticsSession, AuthResult, AuthUser, CreateSessionInput,
    FingerprintProvider, SessionAnalytics, UserEvent, UserEventPublisher, UserProvider,
};

struct StubAnalyticsProvider {
    recent_session: Option<AnalyticsSession>,
    created_sessions: AtomicUsize,
}

impl StubAnalyticsProvider {
    fn new(recent_session: Option<AnalyticsSession>) -> Self {
        Self {
            recent_session,
            created_sessions: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl AnalyticsProvider for StubAnalyticsProvider {
    fn extract_analytics(
        &self,
        _headers: &HeaderMap,
        _uri: Option<&http::Uri>,
    ) -> SessionAnalytics {
        SessionAnalytics::default()
    }

    async fn create_session(&self, _input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        self.created_sessions.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn find_recent_session_by_fingerprint(
        &self,
        _fingerprint: &str,
        _max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        Ok(self.recent_session.clone())
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

struct StubFingerprintProvider {
    active_sessions: i64,
    reusable_session: Option<String>,
}

#[async_trait]
impl FingerprintProvider for StubFingerprintProvider {
    async fn count_active_sessions(&self, _fingerprint: &str) -> AnalyticsResult<i64> {
        Ok(self.active_sessions)
    }

    async fn find_reusable_session(&self, _fingerprint: &str) -> AnalyticsResult<Option<String>> {
        Ok(self.reusable_session.clone())
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

struct StubUserProvider {
    known_user: Option<AuthUser>,
}

fn anon_user(id: &str) -> AuthUser {
    AuthUser {
        id: UserId::new(id),
        name: "anonymous".to_owned(),
        email: format!("{id}@example.com"),
        roles: vec![],
        is_active: true,
    }
}

#[async_trait]
impl UserProvider for StubUserProvider {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        Ok(self.known_user.clone())
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
        Ok(anon_user("user_created"))
    }

    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(anon_user("user_anon_fresh"))
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
        Ok(UserId::new("user_federated"))
    }
}

struct RecordingPublisher {
    events: std::sync::Mutex<Vec<String>>,
}

impl UserEventPublisher for RecordingPublisher {
    fn publish_user_event(&self, event: UserEvent) {
        self.events.lock().unwrap().push(format!("{event:?}"));
    }
}

fn recent_session(session_id: &str, user_id: Option<&str>) -> AnalyticsSession {
    AnalyticsSession {
        session_id: SessionId::new(session_id),
        user_id: user_id.map(UserId::new),
        fingerprint: Some("fp-known".to_owned()),
        created_at: Utc::now(),
    }
}

fn client_id() -> ClientId {
    ClientId::new("client_session_lookup_tests")
}

fn anonymous_input<'a>(
    headers: &'a HeaderMap,
    client: &'a ClientId,
) -> CreateAnonymousSessionInput<'a> {
    CreateAnonymousSessionInput {
        headers,
        uri: None,
        client_id: client,
        session_source: SessionSource::Web,
    }
}

#[tokio::test]
async fn session_at_fingerprint_limit_is_reused() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(Some(recent_session(
        "sess_recent",
        Some("user_existing"),
    ))));
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(StubFingerprintProvider {
        active_sessions: 5,
        reusable_session: Some("sess_reusable".to_owned()),
    }));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session");

    assert!(!info.is_new);
    assert_eq!(info.session_id.as_str(), "sess_reusable");
    assert_eq!(info.user_id.as_str(), "user_existing");
    assert_eq!(
        info.jwt_token.split('.').count(),
        3,
        "jwt is header.payload.signature"
    );
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn recent_session_is_returned_without_creating_a_new_one() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(Some(recent_session(
        "sess_recent",
        Some("user_existing"),
    ))));
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(StubFingerprintProvider {
        active_sessions: 1,
        reusable_session: None,
    }));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session");

    assert!(!info.is_new);
    assert_eq!(info.session_id.as_str(), "sess_recent");
    assert_eq!(info.user_id.as_str(), "user_existing");
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn recent_session_without_user_falls_through_to_fresh_creation() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(Some(recent_session(
        "sess_recent",
        None,
    ))));
    let publisher = Arc::new(RecordingPublisher {
        events: std::sync::Mutex::new(Vec::new()),
    });
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_event_publisher(Arc::clone(&publisher) as Arc<dyn UserEventPublisher>);

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session");

    assert!(info.is_new);
    assert_eq!(info.user_id.as_str(), "user_anon_fresh");
    assert!(info.session_id.as_str().starts_with("sess_"));
    assert_eq!(
        info.jwt_token.split('.').count(),
        3,
        "jwt is header.payload.signature"
    );
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 1);
    assert!(!publisher.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn create_authenticated_session_persists_for_known_user() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(None));
    let user_id = UserId::new("user_known");
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider {
            known_user: Some(anon_user("user_known")),
        }),
    );

    let session_id = service
        .create_authenticated_session(&user_id, &HeaderMap::new(), SessionSource::Web)
        .await
        .expect("session");

    assert!(session_id.as_str().starts_with("sess_"));
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn create_authenticated_session_rejects_unknown_user() {
    ensure_test_bootstrap();

    let service = SessionCreationService::new(
        Arc::new(StubAnalyticsProvider::new(None)) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    );

    let user_id = UserId::new("user_missing");
    let err = service
        .create_authenticated_session(&user_id, &HeaderMap::new(), SessionSource::Web)
        .await
        .expect_err("unknown user");

    assert!(matches!(
        err,
        SessionCreationError::UserNotFound { user_id: ref u } if u.as_str() == "user_missing"
    ));
}

struct FailingAnalyticsProvider {
    created_sessions: AtomicUsize,
}

#[async_trait]
impl AnalyticsProvider for FailingAnalyticsProvider {
    fn extract_analytics(
        &self,
        _headers: &HeaderMap,
        _uri: Option<&http::Uri>,
    ) -> SessionAnalytics {
        SessionAnalytics::default()
    }
    async fn create_session(&self, _input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        self.created_sessions.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn find_recent_session_by_fingerprint(
        &self,
        _fingerprint: &str,
        _max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        Err(systemprompt_traits::AnalyticsProviderError::Internal(
            "lookup exploded".to_owned(),
        ))
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

struct SlowAnalyticsProvider {
    inner: StubAnalyticsProvider,
}

#[async_trait]
impl AnalyticsProvider for SlowAnalyticsProvider {
    fn extract_analytics(
        &self,
        _headers: &HeaderMap,
        _uri: Option<&http::Uri>,
    ) -> SessionAnalytics {
        SessionAnalytics::default()
    }
    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        self.inner.create_session(input).await
    }
    async fn find_recent_session_by_fingerprint(
        &self,
        fingerprint: &str,
        max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        self.inner
            .find_recent_session_by_fingerprint(fingerprint, max_age_seconds)
            .await
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

struct FailingFingerprintProvider;

#[async_trait]
impl FingerprintProvider for FailingFingerprintProvider {
    async fn count_active_sessions(&self, _fingerprint: &str) -> AnalyticsResult<i64> {
        Err(systemprompt_traits::AnalyticsProviderError::Internal(
            "count exploded".to_owned(),
        ))
    }
    async fn find_reusable_session(&self, _fingerprint: &str) -> AnalyticsResult<Option<String>> {
        Err(systemprompt_traits::AnalyticsProviderError::Internal(
            "reusable exploded".to_owned(),
        ))
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

struct ReusableLookupFailsProvider;

#[async_trait]
impl FingerprintProvider for ReusableLookupFailsProvider {
    async fn count_active_sessions(&self, _fingerprint: &str) -> AnalyticsResult<i64> {
        Ok(9)
    }
    async fn find_reusable_session(&self, _fingerprint: &str) -> AnalyticsResult<Option<String>> {
        Err(systemprompt_traits::AnalyticsProviderError::Internal(
            "reusable exploded".to_owned(),
        ))
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

#[tokio::test]
async fn failing_reusable_session_lookup_falls_through_to_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(None));
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(ReusableLookupFailsProvider));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session despite reusable-lookup failure");

    assert!(info.is_new);
}

#[tokio::test]
async fn at_limit_failing_recent_lookup_falls_through_to_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(FailingAnalyticsProvider {
        created_sessions: AtomicUsize::new(0),
    });
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(StubFingerprintProvider {
        active_sessions: 9,
        reusable_session: Some("sess_reusable".to_owned()),
    }));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session despite recent-lookup failure at limit");

    assert!(info.is_new);
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failing_fingerprint_count_falls_through_to_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(None));
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(FailingFingerprintProvider));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session despite fingerprint failure");

    assert!(info.is_new);
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn at_limit_without_reusable_session_creates_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(None));
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(StubFingerprintProvider {
        active_sessions: 5,
        reusable_session: None,
    }));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session");

    assert!(info.is_new);
    assert_eq!(info.user_id.as_str(), "user_anon_fresh");
}

#[tokio::test]
async fn at_limit_with_recent_session_lacking_user_creates_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(StubAnalyticsProvider::new(Some(recent_session(
        "sess_orphan",
        None,
    ))));
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    )
    .with_fingerprint_provider(Arc::new(StubFingerprintProvider {
        active_sessions: 9,
        reusable_session: Some("sess_reusable".to_owned()),
    }));

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session");

    assert!(info.is_new, "orphan recent session must not be reused");
}

#[tokio::test]
async fn failing_analytics_lookup_falls_through_to_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let analytics = Arc::new(FailingAnalyticsProvider {
        created_sessions: AtomicUsize::new(0),
    });
    let service = SessionCreationService::new(
        Arc::clone(&analytics) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    );

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session despite analytics failure");

    assert!(info.is_new);
    assert_eq!(analytics.created_sessions.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn slow_session_lookup_times_out_and_creates_fresh_session() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let service = SessionCreationService::new(
        Arc::new(SlowAnalyticsProvider {
            inner: StubAnalyticsProvider::new(Some(recent_session("sess_slow", Some("user_slow")))),
        }) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    );

    let headers = HeaderMap::new();
    let client = client_id();
    let info = service
        .create_anonymous_session(anonymous_input(&headers, &client))
        .await
        .expect("session despite lookup timeout");

    assert!(info.is_new, "timed-out lookup must not reuse the session");
    assert_ne!(info.session_id.as_str(), "sess_slow");
}

#[tokio::test]
async fn ensure_anonymous_user_resolves_user_and_fingerprint() {
    ensure_test_bootstrap();

    let service = SessionCreationService::new(
        Arc::new(StubAnalyticsProvider::new(None)) as Arc<dyn AnalyticsProvider>,
        Arc::new(StubUserProvider { known_user: None }),
    );

    let (user_id, fingerprint) = service
        .ensure_anonymous_user(&HeaderMap::new(), None)
        .await
        .expect("anonymous user");

    assert_eq!(user_id.as_str(), "user_anon_fresh");
    assert!(!fingerprint.is_empty());
}
