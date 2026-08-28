//! Ranked-list section payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSectionData {
    pub lists: Vec<ItemList>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ItemList {
    pub title: String,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListItem {
    pub rank: i32,
    pub label: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
}

impl ListSectionData {
    pub const fn new(lists: Vec<ItemList>) -> Self {
        Self { lists }
    }
}

impl ItemList {
    pub fn new(title: impl Into<String>, items: Vec<ListItem>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

impl ListItem {
    pub fn new(rank: i32, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            rank,
            label: label.into(),
            value: value.into(),
            badge: None,
        }
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}
