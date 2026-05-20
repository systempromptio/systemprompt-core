//! Database configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: String,

    #[serde(default)]
    pub external_db_access: bool,
}
