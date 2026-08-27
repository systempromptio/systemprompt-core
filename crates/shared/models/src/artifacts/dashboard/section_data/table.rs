//! Table section payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableSectionData {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sortable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_sort: Option<SortConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SortConfig {
    pub column: String,
    pub order: String,
}

impl TableSectionData {
    pub const fn new(columns: Vec<String>, rows: Vec<serde_json::Value>) -> Self {
        Self {
            columns,
            rows,
            sortable: None,
            default_sort: None,
        }
    }

    pub const fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = Some(sortable);
        self
    }

    pub fn with_default_sort(
        mut self,
        column: impl Into<String>,
        order: impl Into<String>,
    ) -> Self {
        self.default_sort = Some(SortConfig {
            column: column.into(),
            order: order.into(),
        });
        self
    }
}
