//! Router state for the Slack surface, built once from the extension
//! context: the live services view, the platform secret lookup (built-in and
//! custom names), the background-task owner, the messaging dispatcher slot, the
//! reply client for Slack's configured Web API, and the untrusted-guarded
//! client for the payload-supplied `response_url`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_client::{GuardedClient, GuardedClientConfig, guarded_client};
use systemprompt_extension::{ExtensionContext, LateBound};
use systemprompt_manifest::services::ServicesView;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_traits::{
    BackgroundTasks, DispatchOutcome, MessagingDispatchError, MessagingDispatcher,
    MessagingInbound, SecretStore,
};

#[derive(Debug, thiserror::Error)]
pub enum SlackStateError {
    #[error("could not build the slack HTTP clients")]
    Client(#[from] reqwest::Error),
    #[error("the messaging dispatcher slot is missing from the extension state")]
    Dispatcher(#[from] MissingDependency),
}

#[derive(Clone)]
pub struct SlackState {
    services: ServicesView,
    secrets: Arc<dyn SecretStore>,
    background: BackgroundTasks,
    dispatcher: LateBound<dyn MessagingDispatcher>,
    reply_client: reqwest::Client,
    payload_url_client: GuardedClient,
}

impl std::fmt::Debug for SlackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackState").finish_non_exhaustive()
    }
}

impl SlackState {
    pub fn from_context(ctx: &dyn ExtensionContext) -> Result<Self, SlackStateError> {
        let reply_client = guarded_client(&GuardedClientConfig::for_configured())?;
        let payload_url_client = GuardedClient::untrusted(&GuardedClientConfig::for_untrusted())?;
        Self::with_clients(ctx, reply_client, payload_url_client)
    }

    pub fn with_clients(
        ctx: &dyn ExtensionContext,
        reply_client: reqwest::Client,
        payload_url_client: GuardedClient,
    ) -> Result<Self, SlackStateError> {
        let state = ctx.extension_state();
        let dispatcher = state.get::<LateBound<dyn MessagingDispatcher>>()?.clone();
        Ok(Self {
            services: ctx.services(),
            secrets: ctx.secrets(),
            background: ctx.background_tasks(),
            dispatcher,
            reply_client,
            payload_url_client,
        })
    }

    pub(super) const fn services(&self) -> &ServicesView {
        &self.services
    }

    pub(super) fn secrets(&self) -> &dyn SecretStore {
        self.secrets.as_ref()
    }

    pub(super) const fn background(&self) -> &BackgroundTasks {
        &self.background
    }

    pub(super) const fn reply_client(&self) -> &reqwest::Client {
        &self.reply_client
    }

    pub(super) const fn payload_url_client(&self) -> &GuardedClient {
        &self.payload_url_client
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
}
