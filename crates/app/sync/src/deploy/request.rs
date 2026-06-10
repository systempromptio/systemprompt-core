//! Typed inputs and outputs for [`super::DeployOrchestrator`].

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
    pub hostname: Option<String>,
    pub secrets_path: PathBuf,
    pub signing_key_path: PathBuf,
    pub options: DeployOptions,
}

#[derive(Debug, Clone, Copy)]
pub struct DeployOptions {
    pub skip_push: bool,
    pub dry_run: bool,
    /// `None` runs the pipeline without any pre-sync phase (and without the
    /// skip warnings) — used for the initial deploy of a freshly provisioned
    /// tenant, where no runtime files exist yet to lose.
    pub pre_sync: Option<PreSyncOptions>,
}

#[derive(Debug, Clone, Copy)]
pub struct PreSyncOptions {
    pub no_sync: bool,
    pub assume_yes: bool,
}

#[derive(Debug)]
pub struct DeployReport {
    pub outcome: DeployOutcome,
}

#[derive(Debug)]
pub enum DeployOutcome {
    DryRun,
    Deployed {
        image: String,
        status: String,
        app_url: Option<String>,
    },
}
