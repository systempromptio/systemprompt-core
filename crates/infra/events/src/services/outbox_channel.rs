//! The outbox's `NOTIFY` channels and the [`OutboxChannel`] a row is
//! relayed on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const OUTBOX_CHANNEL: &str = "systemprompt_events";
pub const NOTIFICATION_FACTS_CHANNEL: &str = "systemprompt_notification_facts";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxChannel {
    AgUi,
    A2A,
    System,
    Analytics,
    Topic,
    Fact,
}

impl OutboxChannel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgUi => "agui",
            Self::A2A => "a2a",
            Self::System => "system",
            Self::Analytics => "analytics",
            Self::Topic => "topic",
            Self::Fact => "fact",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agui" => Some(Self::AgUi),
            "a2a" => Some(Self::A2A),
            "system" => Some(Self::System),
            "analytics" => Some(Self::Analytics),
            "topic" => Some(Self::Topic),
            "fact" => Some(Self::Fact),
            _ => None,
        }
    }
}
