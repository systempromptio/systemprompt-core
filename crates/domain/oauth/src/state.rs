//! Shared OAuth runtime state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::OAuthRepository;
use std::sync::Arc;
use systemprompt_traits::{
    AnalyticsProvider, FingerprintProvider, McpRegistryProvider, SessionProvider, UserProvider,
};

#[derive(Clone)]
pub struct OAuthState {
    oauth_repository: OAuthRepository,
    analytics_provider: Arc<dyn AnalyticsProvider>,
    session_provider: Arc<dyn SessionProvider>,
    user_provider: Arc<dyn UserProvider>,
    fingerprint_provider: Option<Arc<dyn FingerprintProvider>>,
    mcp_registry: Option<Arc<dyn McpRegistryProvider>>,
}

impl std::fmt::Debug for OAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthState")
            .field("oauth_repository", &"OAuthRepository")
            .field("analytics_provider", &"<provider>")
            .field("session_provider", &"<provider>")
            .field("user_provider", &"<provider>")
            .field(
                "fingerprint_provider",
                &self.fingerprint_provider.as_ref().map(|_| "<provider>"),
            )
            .field(
                "mcp_registry",
                &self.mcp_registry.as_ref().map(|_| "<registry>"),
            )
            .finish()
    }
}

impl OAuthState {
    #[must_use]
    pub fn new(
        oauth_repository: OAuthRepository,
        analytics_provider: Arc<dyn AnalyticsProvider>,
        session_provider: Arc<dyn SessionProvider>,
        user_provider: Arc<dyn UserProvider>,
    ) -> Self {
        Self {
            oauth_repository,
            analytics_provider,
            session_provider,
            user_provider,
            fingerprint_provider: None,
            mcp_registry: None,
        }
    }

    #[must_use]
    pub fn with_mcp_registry(mut self, registry: Arc<dyn McpRegistryProvider>) -> Self {
        self.mcp_registry = Some(registry);
        self
    }

    pub fn mcp_registry(&self) -> Option<&Arc<dyn McpRegistryProvider>> {
        self.mcp_registry.as_ref()
    }

    #[must_use]
    pub fn with_fingerprint_provider(mut self, provider: Arc<dyn FingerprintProvider>) -> Self {
        self.fingerprint_provider = Some(provider);
        self
    }

    pub const fn oauth_repository(&self) -> &OAuthRepository {
        &self.oauth_repository
    }

    pub fn analytics_provider(&self) -> &Arc<dyn AnalyticsProvider> {
        &self.analytics_provider
    }

    pub fn session_provider(&self) -> &Arc<dyn SessionProvider> {
        &self.session_provider
    }

    pub fn user_provider(&self) -> &Arc<dyn UserProvider> {
        &self.user_provider
    }

    pub fn fingerprint_provider(&self) -> Option<&Arc<dyn FingerprintProvider>> {
        self.fingerprint_provider.as_ref()
    }
}
