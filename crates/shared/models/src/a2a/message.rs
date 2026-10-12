//! A2A message and part wire types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_field_names,
    reason = "A2A wire schema names these fields message_id/task_id/context_id — public protocol \
              vocabulary"
)]
pub struct Message {
    pub role: MessageRole,
    pub parts: Vec<Part>,
    pub message_id: MessageId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub context_id: ContextId,
    // JSON: A2A `Message.metadata` is spec-defined as a free-form object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_task_ids: Option<Vec<TaskId>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    #[serde(rename = "ROLE_USER")]
    User,
    #[serde(rename = "ROLE_AGENT")]
    Agent,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", try_from = "PartWire")]
pub struct Part {
    #[serde(flatten)]
    pub content: PartContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    // JSON: A2A `Part.metadata` is spec-defined as a free-form object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PartContent {
    Text(String),
    Raw(String),
    Url(String),
    // JSON: A2A `Part.data` is spec-defined as an arbitrary JSON value.
    Data(serde_json::Value),
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PartContentError {
    #[error("A2A part carries no content: exactly one of text, raw, url or data is required")]
    Missing,
    #[error(
        "A2A part carries {count} content fields: exactly one of text, raw, url or data is allowed"
    )]
    Multiple { count: usize },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartWire {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    url: Option<String>,
    // JSON: A2A `Part.data` is spec-defined as an arbitrary JSON value, including null.
    #[serde(default, deserialize_with = "present_value")]
    data: Option<serde_json::Value>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
    // JSON: A2A `Part.metadata` is spec-defined as a free-form object.
    #[serde(default)]
    metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

// JSON: a present `data` key holds an arbitrary JSON value, including null.
fn present_value<'de, D>(deserializer: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde_json::Value::deserialize(deserializer).map(Some)
}

impl TryFrom<PartWire> for Part {
    type Error = PartContentError;

    fn try_from(wire: PartWire) -> Result<Self, Self::Error> {
        let mut contents = [
            wire.text.map(PartContent::Text),
            wire.raw.map(PartContent::Raw),
            wire.url.map(PartContent::Url),
            wire.data.map(PartContent::Data),
        ]
        .into_iter()
        .flatten();
        let content = contents.next().ok_or(PartContentError::Missing)?;
        let extra = contents.count();
        if extra > 0 {
            return Err(PartContentError::Multiple { count: extra + 1 });
        }
        Ok(Self {
            content,
            filename: wire.filename,
            media_type: wire.media_type,
            metadata: wire.metadata,
        })
    }
}

impl Part {
    const fn from_content(content: PartContent, media_type: Option<String>) -> Self {
        Self {
            content,
            filename: None,
            media_type,
            metadata: None,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::from_content(PartContent::Text(text.into()), None)
    }

    // JSON: A2A `Part.data` is spec-defined as an arbitrary JSON value.
    #[must_use]
    pub const fn data(data: serde_json::Value) -> Self {
        Self::from_content(PartContent::Data(data), None)
    }

    pub fn raw(base64: impl Into<String>, media_type: impl Into<String>) -> Self {
        Self::from_content(PartContent::Raw(base64.into()), Some(media_type.into()))
    }

    pub fn url(url: impl Into<String>, media_type: impl Into<String>) -> Self {
        Self::from_content(PartContent::Url(url.into()), Some(media_type.into()))
    }

    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    #[must_use]
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            PartContent::Text(text) => Some(text),
            _ => None,
        }
    }

    // JSON: A2A `Part.data` is spec-defined as an arbitrary JSON value.
    #[must_use]
    pub const fn as_data(&self) -> Option<&serde_json::Value> {
        match &self.content {
            PartContent::Data(data) => Some(data),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_raw(&self) -> Option<&str> {
        match &self.content {
            PartContent::Raw(raw) => Some(raw),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_url(&self) -> Option<&str> {
        match &self.content {
            PartContent::Url(url) => Some(url),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self.content, PartContent::Raw(_) | PartContent::Url(_))
    }
}
