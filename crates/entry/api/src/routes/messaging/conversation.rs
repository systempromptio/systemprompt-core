//! The chat-platform conversation an inbound message belongs to.
//!
//! Each platform names its organisation, conversation and sender with its own
//! wire ids. The per-platform route keeps them typed in
//! [`MessagingConversation`] so the dispatch core reads them through one place
//! and a Slack id can never stand in for a Teams one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    ContextId, SlackChannelId, SlackUserId, SlackWorkspaceId, TeamsConversationId, TeamsTenantId,
    TeamsUserId,
};

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
