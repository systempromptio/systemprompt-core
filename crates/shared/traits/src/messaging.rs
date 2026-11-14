//! The seam a chat-platform surface dispatches an inbound message through.
//!
//! Slack and Teams differ only at their edges: request verification, payload
//! shape and reply rendering. Everything between (federated identity,
//! authorization, the deterministic conversation context, the per-user A2A
//! token and the blocking `SendMessage` to the agent) is the agent
//! capability's, reached through [`MessagingDispatcher`]. A platform surface
//! normalises its wire payload into a [`MessagingInbound`], calls
//! [`MessagingDispatcher::dispatch`] and renders the [`DispatchOutcome`] back
//! into its own UI.
//!
//! Each platform names its organisation, conversation and sender with its own
//! wire ids. [`MessagingConversation`] keeps them typed, so a Slack id never
//! stands in for a Teams one.
//!
//! The sender's federated claims ride on [`MessagingInbound::sender`]: only
//! the platform surface can read the sender's profile, so a surface that reads
//! nothing passes [`SenderIdentity::Unlinked`] and the sender lands on a
//! fresh, role-less user, never on an existing account.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{
    AgentName, ContextId, SlackChannelId, SlackUserId, SlackWorkspaceId, TeamsConversationId,
    TeamsTenantId, TeamsUserId,
};

use crate::auth::{LinkPolicy, SenderIdentity};

/// Platform-scoped ids of the conversation a message arrived in. A Slack
/// interaction without a channel carries `channel_id: None`.
#[derive(Debug, Clone)]
pub enum MessagingConversation {
    Slack {
        workspace_id: SlackWorkspaceId,
        channel_id: Option<SlackChannelId>,
        user_id: SlackUserId,
    },
    Teams {
        tenant_id: TeamsTenantId,
        conversation_id: TeamsConversationId,
        user_id: TeamsUserId,
    },
}

impl MessagingConversation {
    #[must_use]
    pub const fn platform(&self) -> &'static str {
        match self {
            Self::Slack { .. } => "slack",
            Self::Teams { .. } => "teams",
        }
    }

    #[must_use]
    pub fn channel_key(&self) -> &str {
        match self {
            Self::Slack { channel_id, .. } => {
                channel_id.as_ref().map_or("", SlackChannelId::as_str)
            },
            Self::Teams {
                conversation_id, ..
            } => conversation_id.as_str(),
        }
    }

    #[must_use]
    pub fn sender_wire_id(&self) -> &str {
        match self {
            Self::Slack { user_id, .. } => user_id.as_str(),
            Self::Teams { user_id, .. } => user_id.as_str(),
        }
    }

    #[must_use]
    pub fn context_id(&self) -> ContextId {
        let org = match self {
            Self::Slack { workspace_id, .. } => workspace_id.as_str(),
            Self::Teams { tenant_id, .. } => tenant_id.as_str(),
        };
        ContextId::derived_from_messaging(self.platform(), org, self.channel_key())
    }
}

/// A platform-neutral inbound message ready for dispatch. The platform
/// issuer (`https://slack.com`, the Bot Framework issuer) namespaces the
/// sender's wire id in the federated identity resolution.
#[derive(Debug, Clone)]
pub struct MessagingInbound {
    pub issuer: String,
    pub conversation: MessagingConversation,
    pub text: String,
    pub agent_name: AgentName,
    pub sender: SenderIdentity,
    pub link: LinkPolicy,
}

#[derive(Debug, Clone)]
pub enum DispatchOutcome {
    Replied(String),
    Denied(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MessagingDispatchError {
    #[error("no messaging dispatcher is bound on this node")]
    Unavailable,
    #[error("messaging dispatch failed")]
    Failed(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl MessagingDispatchError {
    pub fn failed(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Failed(Box::new(source))
    }

    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "opaque by contract: no part of the error may reach the caller"
    )]
    pub fn user_message(&self) -> String {
        "Sorry — something went wrong handling that.".to_owned()
    }
}

/// Runs one inbound chat-platform message through identity, authorization
/// and the agent, and returns what to render back. Held as
/// `Arc<dyn MessagingDispatcher>` in the extension state, so it uses
/// `#[async_trait]`; native `async fn` in traits is not `dyn`-compatible.
#[async_trait]
pub trait MessagingDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        inbound: MessagingInbound,
    ) -> Result<DispatchOutcome, MessagingDispatchError>;
}

pub type DynMessagingDispatcher = Arc<dyn MessagingDispatcher>;
