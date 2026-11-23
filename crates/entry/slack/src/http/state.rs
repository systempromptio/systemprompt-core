//! Router state for the Slack surface, resolved from the kernel state: the
//! live services view, the member's secret reads ([`SlackSecrets`]), the
//! background-task owner, the member's dispatch handle
//! ([`SlackDispatch`]), and the
//! member's HTTP clients ([`SlackClients`]) — the reply client for Slack's
//! configured Web API and the untrusted-guarded client for the
//! payload-supplied `response_url`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::verify::SlackVerified;
use systemprompt_client::{GuardedClient, GuardedClientConfig, guarded_client};
use systemprompt_extension::{FromState, LateRead, Resolved};
use systemprompt_manifest::services::{ServicesView, SlackAppConfig};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_traits::{
    BackgroundTasks, DispatchOutcome, MessagingDispatchError, MessagingDispatcher,
    MessagingInbound, SecretStore,
};

/// The Slack member's outbound HTTP clients, built once by its provide fn.
#[derive(Clone)]
pub struct SlackClients {
    reply: reqwest::Client,
    payload_url: GuardedClient,
}

impl std::fmt::Debug for SlackClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackClients").finish_non_exhaustive()
    }
}

impl SlackClients {
    pub fn build() -> Result<Self, reqwest::Error> {
        Ok(Self::new(
            guarded_client(&GuardedClientConfig::for_configured())?,
            GuardedClient::untrusted(&GuardedClientConfig::for_untrusted())?,
        ))
    }

    #[must_use]
    pub const fn new(reply: reqwest::Client, payload_url: GuardedClient) -> Self {
        Self { reply, payload_url }
    }
}

/// The Slack member's secret reads.
///
/// An app's signing secret and bot token by the custom-secret refs its config
/// names, and nothing else. Built only by the member's own provide fn over the
/// kernel's sealed secret store and provided sealed, so only the member's
/// route state reads it.
#[derive(Clone)]
pub struct SlackSecrets {
    secrets: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for SlackSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackSecrets").finish_non_exhaustive()
    }
}

impl SlackSecrets {
    pub(crate) const fn over(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets }
    }

    pub(super) fn signing_secret(&self, app: &SlackAppConfig) -> Option<String> {
        self.read(app.signing_secret_ref.as_str())
    }

    pub(super) fn bot_token(&self, app: &SlackAppConfig) -> Option<String> {
        self.read(app.bot_token_ref.as_str())
    }

    fn read(&self, name: &str) -> Option<String> {
        self.secrets
            .secret(name)
            .map(|secret| secret.expose().to_owned())
    }
}

/// The Slack member's handle on the sealed messaging dispatcher: built only
/// by the member's own provide fn, provided sealed, and driven only with the
/// `SlackVerified` proof the signature verifier returns.
#[derive(Clone)]
pub struct SlackDispatch {
    dispatcher: LateRead<dyn MessagingDispatcher>,
}

impl std::fmt::Debug for SlackDispatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackDispatch")
            .field("bound", &self.dispatcher.is_bound())
            .finish()
    }
}

impl SlackDispatch {
    pub(crate) const fn over(dispatcher: LateRead<dyn MessagingDispatcher>) -> Self {
        Self { dispatcher }
    }

    async fn dispatch(
        &self,
        _verified: SlackVerified,
        inbound: MessagingInbound,
    ) -> Result<DispatchOutcome, MessagingDispatchError> {
        let Some(dispatcher) = self.dispatcher.get() else {
            return Err(MessagingDispatchError::Unavailable);
        };
        dispatcher.dispatch(inbound).await
    }
}

#[derive(Clone)]
pub struct SlackState {
    services: ServicesView,
    secrets: SlackSecrets,
    background: BackgroundTasks,
    dispatch: SlackDispatch,
    reply_client: reqwest::Client,
    payload_url_client: GuardedClient,
}

impl std::fmt::Debug for SlackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackState").finish_non_exhaustive()
    }
}

impl FromState for SlackState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let clients = r.get::<SlackClients>()?;
        Ok(Self {
            services: r.get()?,
            secrets: r.get()?,
            background: r.tasks().clone(),
            dispatch: r.get()?,
            reply_client: clients.reply,
            payload_url_client: clients.payload_url,
        })
    }
}

impl SlackState {
    pub(super) const fn services(&self) -> &ServicesView {
        &self.services
    }

    pub(super) const fn secrets(&self) -> &SlackSecrets {
        &self.secrets
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
        verified: SlackVerified,
        inbound: MessagingInbound,
    ) -> Result<DispatchOutcome, MessagingDispatchError> {
        self.dispatch.dispatch(verified, inbound).await
    }
}
