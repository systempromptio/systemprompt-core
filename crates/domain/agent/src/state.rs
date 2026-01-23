use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::Config;
use systemprompt_traits::{
    AnalyticsProvider, DynFileUploadProvider, DynJwtValidationProvider, DynMcpServiceProvider,
    DynProcessCleanupProvider, DynSessionAnalyticsProvider, DynUserProvider,
};

#[derive(Clone)]
pub struct AgentState {
    db_pool: DbPool,
    config: Arc<Config>,
    jwt_provider: DynJwtValidationProvider,
    user_provider: Option<DynUserProvider>,
    analytics_provider: Option<Arc<dyn AnalyticsProvider>>,
    session_analytics_provider: Option<DynSessionAnalyticsProvider>,
    file_upload_provider: Option<DynFileUploadProvider>,
    mcp_service_provider: Option<DynMcpServiceProvider>,
    process_cleanup_provider: Option<DynProcessCleanupProvider>,
}

impl AgentState {
    #[must_use]
    pub fn new(
        db_pool: DbPool,
        config: Arc<Config>,
        jwt_provider: DynJwtValidationProvider,
    ) -> Self {
        Self {
            db_pool,
            config,
            jwt_provider,
            user_provider: None,
            analytics_provider: None,
            session_analytics_provider: None,
            file_upload_provider: None,
            mcp_service_provider: None,
            process_cleanup_provider: None,
        }
    }

    #[must_use]
    pub fn with_user_provider(mut self, provider: DynUserProvider) -> Self {
        self.user_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_analytics_provider(mut self, provider: Arc<dyn AnalyticsProvider>) -> Self {
        self.analytics_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_session_analytics_provider(
        mut self,
        provider: DynSessionAnalyticsProvider,
    ) -> Self {
        self.session_analytics_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_file_upload_provider(mut self, provider: DynFileUploadProvider) -> Self {
        self.file_upload_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_mcp_service_provider(mut self, provider: DynMcpServiceProvider) -> Self {
        self.mcp_service_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_process_cleanup_provider(mut self, provider: DynProcessCleanupProvider) -> Self {
        self.process_cleanup_provider = Some(provider);
        self
    }

    #[must_use]
    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[must_use]
    pub fn jwt_provider(&self) -> &DynJwtValidationProvider {
        &self.jwt_provider
    }

    #[must_use]
    pub fn user_provider(&self) -> Option<&DynUserProvider> {
        self.user_provider.as_ref()
    }

    #[must_use]
    pub fn analytics_provider(&self) -> Option<&Arc<dyn AnalyticsProvider>> {
        self.analytics_provider.as_ref()
    }

    #[must_use]
    pub fn session_analytics_provider(&self) -> Option<&DynSessionAnalyticsProvider> {
        self.session_analytics_provider.as_ref()
    }

    #[must_use]
    pub fn file_upload_provider(&self) -> Option<&DynFileUploadProvider> {
        self.file_upload_provider.as_ref()
    }

    #[must_use]
    pub fn mcp_service_provider(&self) -> Option<&DynMcpServiceProvider> {
        self.mcp_service_provider.as_ref()
    }

    #[must_use]
    pub fn process_cleanup_provider(&self) -> Option<&DynProcessCleanupProvider> {
        self.process_cleanup_provider.as_ref()
    }
}

impl std::fmt::Debug for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentState")
            .field("db_pool", &"<DbPool>")
            .field("config", &"<Arc<Config>>")
            .field("jwt_provider", &"<DynJwtValidationProvider>")
            .field("user_provider", &self.user_provider.is_some())
            .field("analytics_provider", &self.analytics_provider.is_some())
            .field(
                "session_analytics_provider",
                &self.session_analytics_provider.is_some(),
            )
            .field("file_upload_provider", &self.file_upload_provider.is_some())
            .field("mcp_service_provider", &self.mcp_service_provider.is_some())
            .field(
                "process_cleanup_provider",
                &self.process_cleanup_provider.is_some(),
            )
            .finish()
    }
}
