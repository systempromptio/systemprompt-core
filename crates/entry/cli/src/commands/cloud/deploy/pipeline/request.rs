//! Typed inputs and outputs for [`super::DeployOrchestrator`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_cloud::CloudCredentials;
use systemprompt_identifiers::TenantId;

#[derive(Debug)]
pub struct DeployRequest {
    pub tenant_id: TenantId,
    pub tenant_name: String,
    pub profile_name: String,
    pub project_root: PathBuf,
    pub credentials: CloudCredentials,
    pub secrets_path: PathBuf,
    pub signing_key_path: PathBuf,
    pub options: DeployOptions,
}

#[derive(Debug, Clone, Copy)]
pub struct DeployOptions {
    pub skip_push: bool,
}

#[derive(Debug)]
pub struct DeployReport {
    pub image: String,
    pub status: String,
    pub app_url: Option<String>,
}
