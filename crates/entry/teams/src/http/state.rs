//! Router state for the Teams surface, resolved from the kernel state: the
//! live services view, the platform secret lookup (built-in and
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
use systemprompt_extension::{FromState, LateBound, Resolved};
use systemprompt_identifiers::TeamsAppId;
use systemprompt_manifest::services::{ServicesView, TeamsAppConfig};
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_teams::auth::ActivityTokenVerifier;
use systemprompt_traits::{
    BackgroundTasks, DispatchOutcome, MessagingDispatchError, MessagingDispatcher,
    MessagingInbound, SecretStore,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VerifierKey {
    app_id: TeamsAppId,
    openid_config_url: String,
}

type Verifiers = Arc<Mutex<HashMap<VerifierKey, Arc<ActivityTokenVerifier>>>>;

/// The Teams member's outbound HTTP clients and per-app token verifiers,
/// built once by its provide fn and shared by every resolution.
#[derive(Clone)]
pub struct TeamsClients {
    http: reqwest::Client,
    service_url: GuardedClient,
    verifiers: Verifiers,
}

impl std::fmt::Debug for TeamsClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeamsClients").finish_non_exhaustive()
    }
}

impl TeamsClients {
    pub fn build() -> Result<Self, reqwest::Error> {
        Self::with_service_url_client(GuardedClient::untrusted(
            &GuardedClientConfig::for_untrusted(),
        )?)
    }

    pub fn with_service_url_client(service_url: GuardedClient) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_DEFAULT_TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            service_url,
            verifiers: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

#[derive(Clone)]
pub struct TeamsState {
    services: ServicesView,
    secrets: Arc<dyn SecretStore>,
    background: BackgroundTasks,
    dispatcher: LateBound<dyn MessagingDispatcher>,
    http: reqwest::Client,
    service_url_client: GuardedClient,
    verifiers: Verifiers,
}

impl std::fmt::Debug for TeamsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeamsState").finish_non_exhaustive()
    }
}

impl FromState for TeamsState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let clients = r.get::<TeamsClients>()?;
        Ok(Self {
            services: r.get()?,
            secrets: r.get()?,
            background: r.tasks().clone(),
            dispatcher: r.get()?,
            http: clients.http,
            service_url_client: clients.service_url,
            verifiers: clients.verifiers,
        })
    }
}

impl TeamsState {
    pub(super) const fn services(&self) -> &ServicesView {
        &self.services
    }

    pub(super) fn secrets(&self) -> &dyn SecretStore {
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
