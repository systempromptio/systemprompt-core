//! Attention levels and the reason a recipient received one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId};

/// How loudly a notification asks for a reader: ordered `silent < agent <
/// human`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Attention {
    #[default]
    Silent,
    Agent,
    Human,
}

/// A value that is not one of the notification vocabulary's string forms.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown {field} value: {value}")]
pub struct UnknownNotificationValue {
    pub field: &'static str,
    pub value: String,
    #[source]
    pub source: Option<IdValidationError>,
}

impl UnknownNotificationValue {
    pub fn new(field: &'static str, value: impl Into<String>) -> Self {
        Self {
            field,
            value: value.into(),
            source: None,
        }
    }

    pub fn invalid_id(
        field: &'static str,
        value: impl Into<String>,
        source: IdValidationError,
    ) -> Self {
        Self {
            field,
            value: value.into(),
            source: Some(source),
        }
    }
}

impl Attention {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Silent => "silent",
            Self::Agent => "agent",
            Self::Human => "human",
        }
    }
}

impl fmt::Display for Attention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Attention {
    type Err = UnknownNotificationValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "silent" => Ok(Self::Silent),
            "agent" => Ok(Self::Agent),
            "human" => Ok(Self::Human),
            other => Err(UnknownNotificationValue::new("attention", other)),
        }
    }
}

/// Why a recipient holds the attention it does, stored as
/// `sender | rule:<id> | stream_floor:<id> | stream_ceiling:<id> |
/// subscription_ceiling | quiet_hours | default | escalated`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AttentionReason {
    Sender,
    Rule(NotificationRuleId),
    StreamFloor(NotificationStreamId),
    StreamCeiling(NotificationStreamId),
    SubscriptionCeiling,
    QuietHours,
    Default,
    Escalated,
}

impl fmt::Display for AttentionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sender => f.write_str("sender"),
            Self::Rule(id) => write!(f, "rule:{id}"),
            Self::StreamFloor(id) => write!(f, "stream_floor:{id}"),
            Self::StreamCeiling(id) => write!(f, "stream_ceiling:{id}"),
            Self::SubscriptionCeiling => f.write_str("subscription_ceiling"),
            Self::QuietHours => f.write_str("quiet_hours"),
            Self::Default => f.write_str("default"),
            Self::Escalated => f.write_str("escalated"),
        }
    }
}

impl FromStr for AttentionReason {
    type Err = UnknownNotificationValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let unknown = || UnknownNotificationValue::new("attention_reason", s);
        let invalid =
            |e: IdValidationError| UnknownNotificationValue::invalid_id("attention_reason", s, e);
        match s {
            "sender" => return Ok(Self::Sender),
            "subscription_ceiling" => return Ok(Self::SubscriptionCeiling),
            "quiet_hours" => return Ok(Self::QuietHours),
            "default" => return Ok(Self::Default),
            "escalated" => return Ok(Self::Escalated),
            _ => {},
        }
        let (tag, id) = s.split_once(':').ok_or_else(unknown)?;
        match tag {
            "rule" => NotificationRuleId::try_new(id)
                .map(Self::Rule)
                .map_err(invalid),
            "stream_floor" => NotificationStreamId::try_new(id)
                .map(Self::StreamFloor)
                .map_err(invalid),
            "stream_ceiling" => NotificationStreamId::try_new(id)
                .map(Self::StreamCeiling)
                .map_err(invalid),
            _ => Err(unknown()),
        }
    }
}

crate::notifications::string_form::string_serde!(AttentionReason, "AttentionReason");
