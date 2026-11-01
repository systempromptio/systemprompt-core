//! [`PlaneRow`]: one declared brain entity (a kind, topic, interest,
//! workflow profile or config document) as the config plane stores it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

/// Who last wrote a plane row: the `services/brain/` files or the console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaneSource {
    File,
    Console,
}

impl PlaneSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Console => "console",
        }
    }
}

/// A declared entity keyed by its id, with the declaration as JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneRow {
    pub id: String,
    // JSON: a plane row stores its YAML declaration verbatim as JSONB.
    pub body: serde_json::Value,
    pub source: String,
    pub updated_at: DateTime<Utc>,
}
