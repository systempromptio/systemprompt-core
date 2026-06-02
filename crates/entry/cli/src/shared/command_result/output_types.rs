//! Reusable command-side payload shapes.
//!
//! These structs are convenience data carriers that commands build and then
//! pass to a [`CommandOutput`](super::CommandOutput) constructor (typically
//! `text`/`card_value`). They are plain serializable data, independent of the
//! artifact wire format.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextOutput {
    pub message: String,
}

impl TextOutput {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SuccessOutput {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<String>>,
}

impl SuccessOutput {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = Some(details);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KeyValueOutput {
    pub items: Vec<KeyValueItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KeyValueItem {
    pub key: String,
    pub value: String,
}

impl KeyValueOutput {
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.items.push(KeyValueItem {
            key: key.into(),
            value: value.into(),
        });
        self
    }
}

impl Default for KeyValueOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableOutput<T> {
    pub rows: Vec<T>,
}

impl<T> TableOutput<T> {
    pub const fn new(rows: Vec<T>) -> Self {
        Self { rows }
    }
}

impl<T> Default for TableOutput<T> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}
