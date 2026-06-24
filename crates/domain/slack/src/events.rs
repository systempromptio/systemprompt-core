//! Typed inbound Slack payloads and their normalization.
//!
//! The three inbound surfaces — Events API (JSON), slash commands
//! (form-encoded), and interactivity (a form field carrying JSON) — each have a
//! distinct wire shape. They are deserialized into the structs below and then
//! collapsed into a single [`NormalizedInbound`] that the dispatcher consumes,
//! so downstream identity, authorization, and agent-routing logic is written
//! once.

use serde::Deserialize;
use systemprompt_identifiers::{SlackChannelId, SlackUserId, SlackWorkspaceId};

/// The inbound surface a request arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackSurface {
    Event,
    Command,
    Interaction,
}

/// Outer envelope for the Events API (`application/json`).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventsApiEnvelope {
    /// One-time handshake when configuring the Request URL.
    UrlVerification { challenge: String },
    /// A subscribed workspace event.
    EventCallback {
        team_id: SlackWorkspaceId,
        event: SlackEvent,
    },
}

/// The inner `event` object for message-like events.
#[derive(Debug, Clone, Deserialize)]
pub struct SlackEvent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub user: Option<SlackUserId>,
    #[serde(default)]
    pub channel: Option<SlackChannelId>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub ts: Option<String>,
    /// Set on bot-authored messages; used to drop the bot's own echoes.
    #[serde(default)]
    pub bot_id: Option<String>,
}

/// A slash command (`application/x-www-form-urlencoded`).
#[derive(Debug, Clone, Deserialize)]
pub struct SlashCommand {
    pub command: String,
    #[serde(default)]
    pub text: String,
    pub user_id: SlackUserId,
    pub channel_id: SlackChannelId,
    pub team_id: SlackWorkspaceId,
    pub response_url: String,
}

/// Interactivity payload (the JSON inside the `payload` form field).
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionPayload {
    #[serde(rename = "type")]
    pub kind: String,
    pub user: InteractionUser,
    #[serde(default)]
    pub channel: Option<InteractionChannel>,
    pub team: InteractionTeam,
    #[serde(default)]
    pub response_url: Option<String>,
    #[serde(default)]
    pub actions: Vec<InteractionAction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InteractionUser {
    pub id: SlackUserId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InteractionChannel {
    pub id: SlackChannelId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InteractionTeam {
    pub id: SlackWorkspaceId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InteractionAction {
    #[serde(default)]
    pub action_id: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// A surface-agnostic inbound request ready for dispatch.
#[derive(Debug, Clone)]
pub struct NormalizedInbound {
    pub surface: SlackSurface,
    pub workspace_id: SlackWorkspaceId,
    pub channel_id: SlackChannelId,
    pub slack_user_id: SlackUserId,
    pub text: String,
    /// Routing key looked up against `SlackAppConfig.routing`: the slash
    /// command (`/ask`) for commands, otherwise the channel id.
    pub routing_key: String,
    /// Slack-provided reply URL (commands/interactivity); `None` for events,
    /// which reply via `chat.postMessage`.
    pub response_url: Option<String>,
}

impl SlashCommand {
    #[must_use]
    pub fn normalize(self) -> NormalizedInbound {
        NormalizedInbound {
            surface: SlackSurface::Command,
            workspace_id: self.team_id,
            channel_id: self.channel_id,
            slack_user_id: self.user_id,
            routing_key: self.command.clone(),
            text: self.text,
            response_url: Some(self.response_url),
        }
    }
}
