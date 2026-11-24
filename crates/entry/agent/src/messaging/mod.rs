//! The agent capability's dispatch leg for chat-platform inbound messages.
//!
//! [`A2aMessagingDispatcher`] implements the shared
//! [`MessagingDispatcher`] seam: federated identity, the authorization hook,
//! the deterministic conversation context, per-user A2A token minting, the
//! blocking `SendMessage` through the agent dispatch, and reply extraction.
//! The platform surfaces (the Slack and Teams entry crates) never name this
//! module.
//!
//! The `agent-http` member builds it in its provide fn and provides it
//! **sealed** (`Arc<dyn MessagingDispatcher>`): it mints a token for the user
//! it resolves and sends as them. It reads, there, the host's sealed authz
//! hook cell (decided through [`LateAuthzHook`], which refuses every request
//! until the host binds the composed hook), the signing keys and issuer it
//! mints with ([`A2aTokenIssuer`]), the sealed agent dispatch cell, and a user
//! repository it builds over the database. The kernel binds the sealed
//! messaging cell to it while the server builds its routes; with `agent-http`
//! not admitted the cell stays unbound and every dispatch is refused with
//! `MessagingDispatchError::Unavailable`.
//!
//! The pipeline is synchronous and spawned: the platform route acks within
//! its timeout, then a spawned task runs this blocking dispatch and posts the
//! reply. A stable [`ContextId`](systemprompt_identifiers::ContextId), derived
//! from the conversation, ties multi-turn history together.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod a2a;
pub mod identity;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_governance::authz::{
    AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzHookCell, AuthzRequest, EntityRef,
    LateAuthzHook,
};
use systemprompt_identifiers::{Actor, SessionId, TraceId};
use systemprompt_models::a2a::dispatch::{AgentDispatchCell, AgentDispatchError};
use systemprompt_oauth::OauthError;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{
    DispatchOutcome, MessagingConversation, MessagingDispatchError, MessagingDispatcher,
    MessagingInbound,
};
use systemprompt_users::{UserError, UserRepository};

use a2a::{A2aTokenIssuer, authenticated_user, build_a2a_request, run_agent};
use identity::resolve_or_link_user;

/// Failures along the dispatch pipeline. This is an internal system surface;
/// messages are deliberately descriptive for operator debugging.
#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("identity resolution failed")]
    Identity(#[source] UserError),
    #[error("token minting failed")]
    Token(#[source] OauthError),
    #[error("could not encode the agent request")]
    Encode(#[source] serde_json::Error),
    #[error("agent dispatch failed")]
    Dispatch(#[source] AgentDispatchError),
    #[error("the agent dispatch is not bound")]
    DispatchUnbound,
    #[error("agent returned JSON-RPC error {code}: {message}")]
    AgentRejected { code: i32, message: String },
    #[error("malformed agent response")]
    Response(#[source] serde_json::Error),
}

/// The collaborators [`A2aMessagingDispatcher`] is built from, each read by
/// the `agent-http` provide fn.
pub struct DispatcherInputs {
    pub users: Arc<UserRepository>,
    pub authz: AuthzHookCell,
    pub issuer: String,
    pub keys: SigningKeys,
    pub dispatch: AgentDispatchCell,
}

impl std::fmt::Debug for DispatcherInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DispatcherInputs").finish_non_exhaustive()
    }
}

/// The [`MessagingDispatcher`] the `agent-http` member provides sealed.
pub struct A2aMessagingDispatcher {
    users: Arc<UserRepository>,
    authz: LateAuthzHook,
    tokens: A2aTokenIssuer,
    dispatch: AgentDispatchCell,
}

impl std::fmt::Debug for A2aMessagingDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aMessagingDispatcher")
            .finish_non_exhaustive()
    }
}

impl A2aMessagingDispatcher {
    #[must_use]
    pub fn new(inputs: DispatcherInputs) -> Self {
        Self {
            users: inputs.users,
            authz: LateAuthzHook::new(inputs.authz),
            tokens: A2aTokenIssuer {
                issuer: inputs.issuer,
                keys: inputs.keys,
            },
            dispatch: inputs.dispatch,
        }
    }

    pub async fn run(&self, inbound: MessagingInbound) -> Result<DispatchOutcome, MessagingError> {
        let user = match resolve_or_link_user(
            &self.users,
            &inbound.issuer,
            inbound.conversation.sender_wire_id(),
            &inbound.sender.claims(),
            &inbound.link,
        )
        .await
        {
            Ok(user) => user,
            Err(MessagingError::Identity(UserError::FederatedRefused(refusal))) => {
                tracing::warn!(issuer = %inbound.issuer, %refusal, "messaging sender refused");
                return Ok(DispatchOutcome::Denied(refusal.to_string()));
            },
            Err(err) => return Err(err),
        };
        let authed = authenticated_user(&user);

        let context_id = inbound.conversation.context_id();

        let authz = AuthzRequest {
            entity: authz_entity(&inbound.conversation),
            user_id: user.id.clone(),
            actor: Some(Actor::user(user.id.clone())),
            client_id: None,
            access_scope: None,
            roles: user.roles.clone(),
            attributes: std::collections::BTreeMap::new(),
            trace_id: TraceId::generate(),
            session_id: None,
            context: AuthzContext::extension(
                format!("{}.message", inbound.conversation.platform()),
                json!({ "channel": inbound.conversation.channel_key() }),
            ),
            context_id: Some(context_id.clone()),
            task_id: None,
            act_chain: Vec::new(),
        };
        if let AuthzDecision::Deny { reason, policy } = self.authz.evaluate(authz).await {
            return Ok(DispatchOutcome::Denied(format!("{policy}: {reason}")));
        }

        let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
        let token = self.tokens.mint(&authed, &session_id)?;

        let request = build_a2a_request(&inbound, &authed, &session_id, &token, &context_id)?;
        let dispatch = self
            .dispatch
            .get()
            .cloned()
            .ok_or(MessagingError::DispatchUnbound)?;
        let reply = run_agent(dispatch.as_ref(), &inbound.agent_name, request).await?;
        Ok(DispatchOutcome::Replied(reply))
    }
}

#[async_trait]
impl MessagingDispatcher for A2aMessagingDispatcher {
    async fn dispatch(
        &self,
        inbound: MessagingInbound,
    ) -> Result<DispatchOutcome, MessagingDispatchError> {
        Box::pin(self.run(inbound))
            .await
            .map_err(MessagingDispatchError::failed)
    }
}

/// The dispatcher `agent-http` provides on a host that seeds no signing keys
/// or agent dispatch (a child host): every dispatch is refused.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMessagingDispatch;

#[async_trait]
impl MessagingDispatcher for NoMessagingDispatch {
    async fn dispatch(
        &self,
        _inbound: MessagingInbound,
    ) -> Result<DispatchOutcome, MessagingDispatchError> {
        Err(MessagingDispatchError::Unavailable)
    }
}

fn authz_entity(conversation: &MessagingConversation) -> EntityRef {
    match conversation {
        MessagingConversation::Slack { workspace_id, .. } => {
            EntityRef::SlackWorkspace(workspace_id.clone())
        },
        MessagingConversation::Teams { tenant_id, .. } => EntityRef::TeamsTenant(tenant_id.clone()),
    }
}
