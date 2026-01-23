use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::{
    AnalyticsProvider, FingerprintProvider, UserEventPublisher, UserProvider,
};

#[derive(Clone)]
pub struct OAuthState {
    db_pool: DbPool,
    analytics_provider: Arc<dyn AnalyticsProvider>,
    user_provider: Arc<dyn UserProvider>,
    fingerprint_provider: Option<Arc<dyn FingerprintProvider>>,
    event_publisher: Option<Arc<dyn UserEventPublisher>>,
}

impl std::fmt::Debug for OAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthState")
            .field("db_pool", &"DbPool")
            .field("analytics_provider", &"<provider>")
            .field("user_provider", &"<provider>")
            .field(
                "fingerprint_provider",
                &self.fingerprint_provider.as_ref().map(|_| "<provider>"),
            )
            .field(
                "event_publisher",
                &self.event_publisher.as_ref().map(|_| "<publisher>"),
            )
            .finish()
    }
}

impl OAuthState {
    pub fn new(
        db_pool: DbPool,
        analytics_provider: Arc<dyn AnalyticsProvider>,
        user_provider: Arc<dyn UserProvider>,
    ) -> Self {
        Self {
            db_pool,
            analytics_provider,
            user_provider,
            fingerprint_provider: None,
            event_publisher: None,
        }
    }

    #[must_use]
    pub fn with_fingerprint_provider(mut self, provider: Arc<dyn FingerprintProvider>) -> Self {
        self.fingerprint_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_event_publisher(mut self, publisher: Arc<dyn UserEventPublisher>) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    pub fn analytics_provider(&self) -> &Arc<dyn AnalyticsProvider> {
        &self.analytics_provider
    }

    pub fn user_provider(&self) -> &Arc<dyn UserProvider> {
        &self.user_provider
    }

    pub fn fingerprint_provider(&self) -> Option<&Arc<dyn FingerprintProvider>> {
        self.fingerprint_provider.as_ref()
    }

    pub fn event_publisher(&self) -> Option<&Arc<dyn UserEventPublisher>> {
        self.event_publisher.as_ref()
    }
}
