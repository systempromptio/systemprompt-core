//! Extension summary/detail output types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ExtensionId;

use super::capability_types::CapabilitySummary;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSource {
    Compiled,
    Manifest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStanding {
    Core,
    Install,
    Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionSummary {
    pub id: ExtensionId,
    pub name: String,
    pub version: String,
    pub standing: ExtensionStanding,
    pub source: ExtensionSource,
    pub enabled: bool,
    pub capabilities: CapabilitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionListOutput {
    pub extensions: Vec<ExtensionSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobInfo {
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemaInfo {
    pub table: Option<String>,
    pub required_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionDetailOutput {
    pub id: ExtensionId,
    pub name: String,
    pub version: String,
    pub standing: ExtensionStanding,
    pub source: ExtensionSource,
    pub config_prefixes: Vec<String>,
    pub jobs: Vec<JobInfo>,
    pub schemas: Vec<SchemaInfo>,
    pub roles: Vec<RoleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigOutput {
    pub extension_id: ExtensionId,
    pub config_prefix: String,
    // JSON: JSON Schema document for the extension's config block.
    pub config_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigSummary {
    pub extension_id: ExtensionId,
    pub config_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigListOutput {
    pub extensions: Vec<ExtensionConfigSummary>,
    pub total: usize,
}
