//! Per-operation result types reported by [`crate::SyncService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyncOpState {
    NotStarted,
    Partial {
        completed: usize,
        total: usize,
    },
    #[default]
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncOperationResult {
    pub operation: String,
    pub success: bool,
    pub items_synced: usize,
    pub items_skipped: usize,
    pub errors: Vec<String>,
    pub details: Option<serde_json::Value>,
    #[serde(default)]
    pub state: SyncOpState,
}

impl SyncOperationResult {
    pub fn success(operation: &str, items_synced: usize) -> Self {
        Self {
            operation: operation.to_owned(),
            success: true,
            items_synced,
            items_skipped: 0,
            errors: vec![],
            details: None,
            state: SyncOpState::Completed,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn dry_run(operation: &str, items_skipped: usize, details: serde_json::Value) -> Self {
        Self {
            operation: operation.to_owned(),
            success: true,
            items_synced: 0,
            items_skipped,
            errors: vec![],
            details: Some(details),
            state: SyncOpState::Completed,
        }
    }
}
