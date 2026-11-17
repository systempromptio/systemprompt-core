//! Capability summary output types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ExtensionId, JobName};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitySummary {
    pub jobs: usize,
    pub schemas: usize,
    pub roles: usize,
    pub configs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobWithExtension {
    pub extension_id: ExtensionId,
    pub extension_name: String,
    pub job_name: JobName,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobsListOutput {
    pub jobs: Vec<JobWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemaWithExtension {
    pub extension_id: ExtensionId,
    pub extension_name: String,
    pub table: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemasListOutput {
    pub schemas: Vec<SchemaWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleWithExtension {
    pub extension_id: ExtensionId,
    pub extension_name: String,
    pub role_name: String,
    pub display_name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RolesListOutput {
    pub roles: Vec<RoleWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitiesSummaryOutput {
    pub jobs: usize,
    pub schemas: usize,
    pub roles: usize,
    pub configs: usize,
    pub extension_count: usize,
}
