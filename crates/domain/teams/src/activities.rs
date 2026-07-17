//! Typed inbound Bot Framework activities and their normalization.
//!
//! The Azure Bot Service delivers every Teams interaction as an `Activity`
//! object. Two surfaces matter for dispatch: `message` (a user message) and
//! `invoke` (an Adaptive Card action or task-module submit). Both deserialize
//! into [`Activity`] and collapse into a single [`NormalizedInbound`] the
//! dispatcher consumes, so downstream identity, authorization, and
//! agent-routing logic is written once — mirroring the Slack surface
//! normalization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_identifiers::{TeamsConversationId, TeamsTenantId, TeamsUserId};

use crate::error::{TeamsError, TeamsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamsSurface {
    Message,
    Invoke,
}

/// A Bot Framework activity (`application/json`), trimmed to the fields the
/// dispatch path consumes.
#[derive(Debug, Clone, Deserialize)]
pub struct Activity {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    /// Base URL of the channel's Bot Connector — replies POST back here.
    #[serde(rename = "serviceUrl")]
    pub service_url: String,
    #[serde(default)]
    pub text: Option<String>,
    pub from: ActivityAccount,
    pub conversation: ConversationAccount,
    #[serde(rename = "channelData", default)]
    pub channel_data: Option<ChannelData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActivityAccount {
    pub id: String,
    /// Entra (Azure AD) object id of the user, when the channel supplies it.
    #[serde(rename = "aadObjectId", default)]
    pub aad_object_id: Option<TeamsUserId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConversationAccount {
    pub id: TeamsConversationId,
    #[serde(rename = "tenantId", default)]
    pub tenant_id: Option<TeamsTenantId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelData {
    #[serde(default)]
    pub tenant: Option<ChannelDataTenant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelDataTenant {
    pub id: TeamsTenantId,
}

#[derive(Debug, Clone)]
pub struct NormalizedInbound {
    pub surface: TeamsSurface,
    pub tenant_id: TeamsTenantId,
    pub conversation_id: TeamsConversationId,
    pub teams_user_id: TeamsUserId,
    pub text: String,
    /// Routing key looked up against `TeamsAppConfig.routing`: the leading
    /// `/command` token when the message is a command, otherwise the
    /// conversation id.
    pub routing_key: String,
    pub service_url: String,
    pub activity_id: Option<String>,
}

impl Activity {
    #[must_use]
    pub fn surface(&self) -> Option<TeamsSurface> {
        match self.kind.as_str() {
            "message" => Some(TeamsSurface::Message),
            "invoke" => Some(TeamsSurface::Invoke),
            _ => None,
        }
    }

    /// Collapse the activity into a [`NormalizedInbound`].
    ///
    /// Fails with [`TeamsError::MalformedActivity`] when the activity is not a
    /// dispatchable surface or is missing a tenant the governed-identity
    /// mapping requires.
    pub fn normalize(self) -> TeamsResult<NormalizedInbound> {
        let surface = self.surface().ok_or_else(|| {
            TeamsError::MalformedActivity(format!("unhandled type '{}'", self.kind))
        })?;

        let tenant_id = self
            .conversation
            .tenant_id
            .or_else(|| self.channel_data.and_then(|c| c.tenant).map(|t| t.id))
            .ok_or_else(|| TeamsError::MalformedActivity("missing tenant id".to_owned()))?;

        let teams_user_id = self
            .from
            .aad_object_id
            .unwrap_or_else(|| TeamsUserId::new(self.from.id));
        let text = self.text.unwrap_or_default();
        let routing_key = command_token(&text).map_or_else(
            || self.conversation.id.as_str().to_owned(),
            ToOwned::to_owned,
        );

        Ok(NormalizedInbound {
            surface,
            tenant_id,
            conversation_id: self.conversation.id,
            teams_user_id,
            text,
            routing_key,
            service_url: self.service_url,
            activity_id: self.id,
        })
    }
}

fn command_token(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }
    Some(trimmed.split_whitespace().next().unwrap_or(trimmed))
}
