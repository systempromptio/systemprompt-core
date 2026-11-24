//! The agent capability's dispatch leg for chat-platform inbound messages.
//!
//! [`A2aMessagingDispatcher`] implements the shared
//! [`MessagingDispatcher`] seam: federated identity, the authorization hook,
//! the deterministic conversation context, per-user A2A token minting, the
//! blocking `SendMessage` through the agent proxy, and reply extraction. The
//! platform surfaces (the Slack and Teams entry crates) never name this
//! module. The kernel seeds the read half of the dispatcher cell
//! (`LateRead<dyn MessagingDispatcher>`) **sealed**, and [`bind_dispatcher`]
//! binds it, after the agent dispatch it sends through, while the server
//! builds its routes, through the binders the context kept outside the
//! extension state; binders already taken, or a cell already bound, fail the
//! boot. The Slack and Teams members read the sealed read half only in their
//! own provide fns, where they wrap it in a handle that dispatches only a
//! request their platform verifier has verified. A cell that is never bound
//! refuses every dispatch with [`MessagingDispatchError::Unavailable`].
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
use systemprompt_extension::AlreadyBound;
use systemprompt_governance::authz::{AuthzContext, AuthzDecision, AuthzRequest, EntityRef};
use systemprompt_identifiers::{Actor, SessionId, TraceId};
use systemprompt_oauth::OauthError;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    DispatchOutcome, MessagingConversation, MessagingDispatchError, MessagingDispatcher,
    MessagingInbound,
};
use systemprompt_users::UserError;

use a2a::{authenticated_user, build_a2a_request, mint_a2a_token, run_agent};
use identity::resolve_or_link_user;
use systemprompt_models::a2a::dispatch::{AgentDispatchCell, AgentDispatchError};

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

/// The [`MessagingDispatcher`] the API binds into the extension state.
#[derive(Clone)]
pub struct A2aMessagingDispatcher {
    ctx: AppContext,
}

impl std::fmt::Debug for A2aMessagingDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aMessagingDispatcher")
            .finish_non_exhaustive()
    }
}

impl A2aMessagingDispatcher {
    #[must_use]
    pub const fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    pub async fn run(&self, inbound: MessagingInbound) -> Result<DispatchOutcome, MessagingError> {
        let ctx = &self.ctx;
        let user = match resolve_or_link_user(
            ctx,
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
        if let AuthzDecision::Deny { reason, policy } = ctx.authz_hook().evaluate(authz).await {
            return Ok(DispatchOutcome::Denied(format!("{policy}: {reason}")));
        }

        let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
        let token = mint_a2a_token(ctx, &authed, &session_id)?;

        let request = build_a2a_request(&inbound, &authed, &session_id, &token, &context_id)?;
        let dispatch = ctx
            .kernel()
            .state()
            .find::<AgentDispatchCell>()
            .and_then(|cell| cell.get().cloned())
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

/// Binding the agent dispatch and the messaging dispatcher failed; the
/// server refuses to boot.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum DispatcherBindError {
    #[error("the late binders were already taken from this context")]
    BindersTaken,
    #[error("a dispatch cell was already bound")]
    AlreadyBound(#[from] AlreadyBound),
}

pub fn bind_dispatcher(ctx: &AppContext) -> Result<(), DispatcherBindError> {
    let binders = ctx
        .take_late_binders()
        .ok_or(DispatcherBindError::BindersTaken)?;
    crate::services::agent_dispatch::bind_agent_dispatch(ctx, binders.agent_dispatch)?;
    binders
        .messaging
        .bind(Arc::new(A2aMessagingDispatcher::new(ctx.clone())))?;
    Ok(())
}

fn authz_entity(conversation: &MessagingConversation) -> EntityRef {
    match conversation {
        MessagingConversation::Slack { workspace_id, .. } => {
            EntityRef::SlackWorkspace(workspace_id.clone())
        },
        MessagingConversation::Teams { tenant_id, .. } => EntityRef::TeamsTenant(tenant_id.clone()),
    }
}
