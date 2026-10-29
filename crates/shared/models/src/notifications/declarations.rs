//! Declarative notification config: the YAML shapes under
//! `services/notifications/` and their cross-reference validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use chrono::{NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId};

use super::attention::Attention;
use super::subject::{AudienceSubject, SubjectRef};

/// A declared stream: who reads it, who may publish, and its attention band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamDecl {
    pub id: NotificationStreamId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub audience: Vec<AudienceSubject>,
    pub publishers: Vec<AudienceSubject>,
    #[serde(default)]
    pub attention: StreamAttention,
    #[serde(default)]
    pub retention_days: Option<u32>,
    #[serde(default)]
    pub coalesce_window_secs: Option<u32>,
}

/// A stream's default attention and the band every recipient is clamped to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StreamAttention {
    pub default: Attention,
    pub floor: Option<Attention>,
    pub ceiling: Option<Attention>,
}

/// A declared subscription of an audience to a stream or a resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionDecl {
    pub subject: AudienceSubject,
    pub target: SubscriptionTarget,
    #[serde(default)]
    pub attention_ceiling: Option<Attention>,
    #[serde(default)]
    pub quiet_hours: Option<QuietHours>,
    #[serde(default)]
    pub digest: Option<DigestWindow>,
}

/// What a subscription follows: `{stream: <id>}` or `{resource: {kind, id}}`.
///
/// It reads and writes as a one-key map rather than a tagged enum, so YAML
/// declarations use the plain map form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TargetMap", into = "TargetMap")]
pub enum SubscriptionTarget {
    Stream(NotificationStreamId),
    Resource(SubjectRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stream: Option<NotificationStreamId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resource: Option<SubjectRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a subscription target names exactly one of `stream` or `resource`")]
pub struct SubscriptionTargetError;

impl TryFrom<TargetMap> for SubscriptionTarget {
    type Error = SubscriptionTargetError;

    fn try_from(map: TargetMap) -> Result<Self, Self::Error> {
        match (map.stream, map.resource) {
            (Some(stream), None) => Ok(Self::Stream(stream)),
            (None, Some(resource)) => Ok(Self::Resource(resource)),
            _ => Err(SubscriptionTargetError),
        }
    }
}

impl From<SubscriptionTarget> for TargetMap {
    fn from(target: SubscriptionTarget) -> Self {
        match target {
            SubscriptionTarget::Stream(stream) => Self {
                stream: Some(stream),
                resource: None,
            },
            SubscriptionTarget::Resource(resource) => Self {
                stream: None,
                resource: Some(resource),
            },
        }
    }
}

/// A daily window, in a named time zone, during which delivery is held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuietHours {
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub tz: String,
    #[serde(default)]
    pub weekdays: Option<Vec<Weekday>>,
}

/// Batch delivery into one digest per window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DigestWindow {
    pub every_secs: u32,
}

/// A declared rule: when its match holds, its action adjusts delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDecl {
    pub id: NotificationRuleId,
    pub name: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub r#match: RuleMatch,
    #[serde(default)]
    pub action: RuleAction,
}

/// Rule predicates; an empty or absent field matches everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuleMatch {
    pub kinds: Vec<String>,
    pub streams: Vec<NotificationStreamId>,
    pub resources: Vec<SubjectRef>,
    pub actors: Vec<AudienceSubject>,
    pub actor_is_agent: Option<bool>,
    pub recipients: Vec<AudienceSubject>,
}

/// What a matching rule does to a recipient's delivery.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuleAction {
    pub set: Option<Attention>,
    pub cap: Option<Attention>,
    pub floor: Option<Attention>,
    pub mute: bool,
    pub coalesce_window_secs: Option<u32>,
    pub digest: Option<DigestWindow>,
    pub quiet_hours: Option<QuietHours>,
}

/// One notifications YAML file, and the merge of all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NotificationsDoc {
    pub streams: Vec<StreamDecl>,
    pub rules: Vec<RuleDecl>,
    pub subscriptions: Vec<SubscriptionDecl>,
}

impl NotificationsDoc {
    pub fn merge(&mut self, other: Self) {
        self.streams.extend(other.streams);
        self.rules.extend(other.rules);
        self.subscriptions.extend(other.subscriptions);
    }
}

/// A declaration set that is internally inconsistent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeclError {
    #[error("stream {0} is declared more than once")]
    DuplicateStream(NotificationStreamId),
    #[error("rule {0} is declared more than once")]
    DuplicateRule(NotificationRuleId),
    #[error("stream {0} has an attention floor above its ceiling")]
    StreamFloorAboveCeiling(NotificationStreamId),
    #[error("rule {0} has an attention floor above its cap")]
    RuleFloorAboveCap(NotificationRuleId),
    #[error("stream {0} declares no audience")]
    EmptyAudience(NotificationStreamId),
    #[error("stream {0} declares no publishers")]
    EmptyPublishers(NotificationStreamId),
    #[error("rule {rule} references undeclared stream {stream}")]
    UnknownRuleStream {
        rule: NotificationRuleId,
        stream: NotificationStreamId,
    },
    #[error("subscription references undeclared stream {0}")]
    UnknownSubscriptionStream(NotificationStreamId),
}

pub fn validate(doc: &NotificationsDoc) -> Result<(), DeclError> {
    let mut streams = HashSet::new();
    for stream in &doc.streams {
        if !streams.insert(&stream.id) {
            return Err(DeclError::DuplicateStream(stream.id.clone()));
        }
        if let (Some(floor), Some(ceiling)) = (stream.attention.floor, stream.attention.ceiling)
            && floor > ceiling
        {
            return Err(DeclError::StreamFloorAboveCeiling(stream.id.clone()));
        }
        if stream.audience.is_empty() {
            return Err(DeclError::EmptyAudience(stream.id.clone()));
        }
        if stream.publishers.is_empty() {
            return Err(DeclError::EmptyPublishers(stream.id.clone()));
        }
    }
    let mut rules = HashSet::new();
    for rule in &doc.rules {
        if !rules.insert(&rule.id) {
            return Err(DeclError::DuplicateRule(rule.id.clone()));
        }
        if let (Some(floor), Some(cap)) = (rule.action.floor, rule.action.cap)
            && floor > cap
        {
            return Err(DeclError::RuleFloorAboveCap(rule.id.clone()));
        }
        if let Some(stream) = rule.r#match.streams.iter().find(|s| !streams.contains(s)) {
            return Err(DeclError::UnknownRuleStream {
                rule: rule.id.clone(),
                stream: stream.clone(),
            });
        }
    }
    for subscription in &doc.subscriptions {
        if let SubscriptionTarget::Stream(stream) = &subscription.target
            && !streams.contains(stream)
        {
            return Err(DeclError::UnknownSubscriptionStream(stream.clone()));
        }
    }
    Ok(())
}
