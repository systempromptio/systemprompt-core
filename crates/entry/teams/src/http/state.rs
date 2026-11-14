//! Router state for the Teams surface, built once from the extension
//! context: the live services view, the platform secret lookup (built-in and
//! custom names), the background-task owner, the messaging dispatcher slot, one
//! outbound HTTP client with connect and request timeouts for
//! operator-configured endpoints, the untrusted-guarded client replies to the
//! payload-supplied `serviceUrl` travel on, and one [`ActivityTokenVerifier`]
//! per configured app so each verifier's JWKS cache survives across activities.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_client::{GuardedClient, GuardedClientConfig};
use systemprompt_extension::{ExtensionContext, LateBound};
use systemprompt_identifiers::TeamsAppId;
use systemprompt_manifest::services::{ServicesView, TeamsAppConfig};
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_teams::auth::ActivityTokenVerifier;
use systemprompt_traits::{
    BackgroundTasks, DispatchOutcome, MessagingDispatchError, MessagingDispatcher,
    MessagingInbound, SecretLookup,
};

#[derive(Debug, thiserror::Error)]
pub enum TeamsStateError {
    #[error("could not build the teams HTTP clients")]
    Client(#[from] reqwest::Error),
    #[error("the messaging dispatcher slot is missing from the extension state")]
    Dispatcher(#[from] MissingDependency),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VerifierKey {
    app_id: TeamsAppId,
    openid_config_url: String,
}

#[derive(Clone)]
pub struct TeamsState {
    services: ServicesView,
    secrets: Arc<dyn SecretLookup>,
    background: BackgroundTasks,
    dispatcher: LateBound<dyn MessagingDispatcher>,
    http: reqwest::Client,
    service_url_client: GuardedClient,
    verifiers: Arc<Mutex<HashMap<VerifierKey, Arc<ActivityTokenVerifier>>>>,
}

impl std::fmt::Debug for TeamsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeamsState").finish_non_exhaustive()
    }
}

impl TeamsState {
    pub fn from_context(ctx: &dyn ExtensionContext) -> Result<Self, TeamsStateError> {
        let service_url_client = GuardedClient::untrusted(&GuardedClientConfig::for_untrusted())?;
        Self::with_service_url_client(ctx, service_url_client)
    }

    pub fn with_service_url_client(
        ctx: &dyn ExtensionContext,
        service_url_client: GuardedClient,
    ) -> Result<Self, TeamsStateError> {
        let http = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_DEFAULT_TIMEOUT)
            .build()?;
        let state = ctx.extension_state();
        let dispatcher = state.get::<LateBound<dyn MessagingDispatcher>>()?.clone();
        let secrets = Arc::clone(state.get::<Arc<dyn SecretLookup>>()?);
        Ok(Self {
            services: ctx.services(),
            secrets,
            background: ctx.background_tasks(),
            dispatcher,
            http,
            service_url_client,
            verifiers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(super) const fn services(&self) -> &ServicesView {
        &self.services
    }

    pub(super) fn secrets(&self) -> &dyn SecretLookup {
        self.secrets.as_ref()
    }

    pub(super) const fn background(&self) -> &BackgroundTasks {
        &self.background
    }

    pub(super) const fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub(super) const fn service_url_client(&self) -> &GuardedClient {
        &self.service_url_client
    }

    pub(super) async fn dispatch(
        &self,
        inbound: MessagingInbound,
    ) -> Result<DispatchOutcome, MessagingDispatchError> {
        let Some(dispatcher) = self.dispatcher.get() else {
            return Err(MessagingDispatchError::Unavailable);
        };
        dispatcher.dispatch(inbound).await
    }

    pub(super) fn verifier(&self, app: &TeamsAppConfig) -> Arc<ActivityTokenVerifier> {
        let key = VerifierKey {
            app_id: app.app_id.clone(),
            openid_config_url: app.endpoints.openid_config_url.clone(),
        };
        let mut verifiers = self
            .verifiers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        Arc::clone(verifiers.entry(key).or_insert_with_key(|key| {
            Arc::new(ActivityTokenVerifier::with_openid_url(
                self.http.clone(),
                key.app_id.clone(),
                key.openid_config_url.clone(),
            ))
        }))
    }
}
