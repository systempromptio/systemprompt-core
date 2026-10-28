//! [`SsoDriver`]: the one place every browser-redirect SSO provider is run.
//!
//! Built once at a composition root from the inventory registries, it owns
//! the state store, the replay guard and the audit sinks, so a provider only
//! speaks its protocol. The flow itself lives beside it: [`super::start`]
//! and [`super::finish`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_manifest::Config;
use systemprompt_oauth::repository::{OAuthRepository, PostgresReplayGuard};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::sso::{
    AuthEventSink, ReplayGuard, SsoError, SsoLoginRefused, SsoLoginSuccess, SsoMetadataDoc,
    SsoProvider,
};
use systemprompt_users::PostgresAuthEventSink;

use crate::repository::SsoRepositories;

use super::cookie::CookieSpec;
use super::registry::{SsoHookContext, SsoProviderError, discover_providers, discover_sinks};
use super::roles::{RoleResolver, discover_role_resolver};

/// Where `start` sends the browser, and the binder cookie to set first.
#[derive(Debug, Clone)]
pub struct StartOutcome {
    pub redirect_url: String,
    pub binder_cookie: CookieSpec,
}

/// How a callback ends. Both arms carry the expired binder cookie to clear.
#[derive(Debug, Clone)]
pub enum FinishOutcome {
    Success {
        access_token: String,
        max_age_secs: i64,
        return_to: String,
        clear_binder: CookieSpec,
    },
    Refused {
        login_path: String,
        reason: &'static str,
        clear_binder: CookieSpec,
    },
}

#[derive(Clone)]
pub struct SsoDriver {
    pub(super) inner: Arc<DriverInner>,
}

pub(super) struct DriverInner {
    pub(super) providers: Vec<Arc<dyn SsoProvider>>,
    pub(super) repo: OAuthRepository,
    pub(super) replay: Arc<dyn ReplayGuard>,
    pub(super) sinks: Vec<Arc<dyn AuthEventSink>>,
    pub(super) roles: Arc<dyn RoleResolver>,
    pub(super) session_service: Arc<SessionCreationService>,
    pub(super) config: Arc<Config>,
}

impl std::fmt::Debug for SsoDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoDriver")
            .field("providers", &self.provider_ids())
            .field("sinks", &self.inner.sinks.len())
            .finish_non_exhaustive()
    }
}

impl SsoDriver {
    pub fn discover(
        ctx: &SsoHookContext,
        oauth: OAuthRepository,
        session_service: Arc<SessionCreationService>,
    ) -> Result<Self, SsoProviderError> {
        let db = &ctx.db;
        let config = Arc::clone(&ctx.config);
        let SsoRepositories {
            oauth: repo,
            login_events,
        } = SsoRepositories::new(db, oauth);
        let mut sinks: Vec<Arc<dyn AuthEventSink>> =
            vec![Arc::new(PostgresAuthEventSink::new(login_events))];
        sinks.extend(discover_sinks(ctx));
        let providers = discover_providers(ctx)?;
        let roles = discover_role_resolver(ctx);
        if !providers.is_empty() {
            tracing::info!(
                providers = ?providers.iter().map(|p| p.id()).collect::<Vec<_>>(),
                "SSO providers registered"
            );
        }
        Ok(Self {
            inner: Arc::new(DriverInner {
                providers,
                replay: Arc::new(PostgresReplayGuard::new(repo.clone())),
                repo,
                sinks,
                roles,
                session_service,
                config,
            }),
        })
    }

    #[must_use]
    pub fn provider(&self, id: &str) -> Option<Arc<dyn SsoProvider>> {
        self.inner
            .providers
            .iter()
            .find(|p| p.id() == id)
            .map(Arc::clone)
    }

    #[must_use]
    pub fn provider_ids(&self) -> Vec<String> {
        self.inner
            .providers
            .iter()
            .map(|p| p.id().to_owned())
            .collect()
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub async fn metadata(&self, id: &str) -> Option<Result<SsoMetadataDoc, SsoError>> {
        self.provider(id)?.metadata().await
    }

    pub(super) async fn emit_success(&self, event: &SsoLoginSuccess<'_>) {
        for sink in &self.inner.sinks {
            sink.on_login_success(event).await;
        }
    }

    pub(super) async fn emit_refused(&self, event: &SsoLoginRefused<'_>) {
        for sink in &self.inner.sinks {
            sink.on_login_refused(event).await;
        }
    }
}
