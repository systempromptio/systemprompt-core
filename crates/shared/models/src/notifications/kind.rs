//! Notification kinds: dotted lowercase names such as `document.shared`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

/// A notification kind: `.`-separated segments of `[a-z0-9_]`, e.g.
/// `document.comment.mentioned` or an extension kind `deal.stage_changed`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NotificationKind(String);

/// A string that is not a valid [`NotificationKind`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid notification kind {value:?}: {reason}")]
pub struct NotificationKindError {
    pub value: String,
    pub reason: &'static str,
}

impl NotificationKind {
    pub const MENTIONED: &'static str = "document.comment.mentioned";
    pub const SHARED: &'static str = "document.shared";
    pub const STREAM_PUBLISHED: &'static str = "stream.published";
    pub const ISSUE_REPORTED: &'static str = "issue.reported";
    pub const ISSUE_STATUS_CHANGED: &'static str = "issue.status_changed";

    pub fn new(value: impl Into<String>) -> Result<Self, NotificationKindError> {
        let value = value.into();
        if let Some(reason) = kind_violation(&value) {
            return Err(NotificationKindError { value, reason });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn kind_violation(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        return Some("must not be empty");
    }
    if value.split('.').any(str::is_empty) {
        return Some("segments must not be empty");
    }
    if !value
        .bytes()
        .all(|b| b == b'.' || b == b'_' || b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return Some("must contain only lowercase letters, digits, '_' and '.'");
    }
    None
}

impl fmt::Display for NotificationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for NotificationKind {
    type Err = NotificationKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

crate::notifications::string_form::string_serde!(NotificationKind, "NotificationKind");
