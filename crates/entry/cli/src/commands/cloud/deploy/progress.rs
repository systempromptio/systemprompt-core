//! CLI rendering for the deploy pipeline.
//!
//! [`CliDeployProgress`] implements the orchestrator's
//! [`DeployProgress`] seam over `CliService` spinners and message sinks:
//! sequencing lives in `systemprompt-sync`, presentation lives here. A single
//! spinner slot is cleared at every event boundary so each long-running step
//! replaces the previous indicator.

use std::sync::Mutex;

use indicatif::ProgressBar;
use systemprompt_logging::CliService;
use systemprompt_sync::deploy::{DeployEvent, DeployProgress, DeployPrompt};
use systemprompt_sync::{
    FileDiffStatus, SyncDiffResult, SyncError, SyncOperationResult, SyncResult,
};

use crate::cli_settings::CliConfig;
use crate::interactive::{Prompter, confirm_optional};

pub struct CliDeployProgress<'a> {
    config: Option<(&'a dyn Prompter, &'a CliConfig)>,
    spinner: Mutex<Option<ProgressBar>>,
}

impl std::fmt::Debug for CliDeployProgress<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliDeployProgress")
            .field("interactive", &self.config.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> CliDeployProgress<'a> {
    pub const fn new(prompter: &'a dyn Prompter, config: &'a CliConfig) -> Self {
        Self {
            config: Some((prompter, config)),
            spinner: Mutex::new(None),
        }
    }

    pub const fn non_interactive() -> Self {
        Self {
            config: None,
            spinner: Mutex::new(None),
        }
    }

    fn start_spinner(&self, message: &str) {
        if let Ok(mut slot) = self.spinner.lock() {
            *slot = Some(CliService::spinner(message));
        }
    }

    fn clear_spinner(&self) {
        if let Ok(mut slot) = self.spinner.lock()
            && let Some(spinner) = slot.take()
        {
            spinner.finish_and_clear();
        }
    }
}

impl DeployProgress for CliDeployProgress<'_> {
    fn event(&self, event: &DeployEvent<'_>) {
        self.clear_spinner();
        if let Some(message) = spinner_message(event) {
            self.start_spinner(message);
            return;
        }
        render_event(event);
    }

    fn confirm(&self, prompt: &DeployPrompt) -> SyncResult<bool> {
        self.clear_spinner();
        let (message, default) = match prompt {
            DeployPrompt::PreSync => ("Sync files from cloud before deploying?".to_owned(), true),
            DeployPrompt::ApplyChanges { count } => (
                format!(
                    "Apply {} change{} from cloud? (backup saved)",
                    count,
                    if *count == 1 { "" } else { "s" }
                ),
                true,
            ),
        };
        self.config.map_or(Ok(default), |(prompter, config)| {
            confirm_optional(prompter, &message, default, config).map_err(SyncError::internal)
        })
    }
}

pub const fn spinner_message(event: &DeployEvent<'_>) -> Option<&'static str> {
    match event {
        DeployEvent::SyncDryRunStarted => Some("Syncing files from cloud..."),
        DeployEvent::SyncDownloadStarted => Some("Downloading files from cloud..."),
        DeployEvent::SyncBackupStarted => Some("Backing up local services..."),
        DeployEvent::RegistryAuthStarted => Some("Fetching registry credentials..."),
        DeployEvent::BuildStarted => Some("Building Docker image..."),
        DeployEvent::PushStarted => Some("Pushing to registry..."),
        DeployEvent::SecretsSyncStarted => Some("Syncing secrets..."),
        DeployEvent::CredentialsSyncStarted => Some("Syncing cloud credentials..."),
        DeployEvent::DeployStarted => Some("Deploying..."),
        _ => None,
    }
}

fn render_event(event: &DeployEvent<'_>) {
    match event {
        DeployEvent::PreSyncSkippedByFlag => {
            CliService::warning("Pre-deploy sync skipped (--no-sync)");
            CliService::warning("Runtime files on the container will be LOST");
        },
        DeployEvent::PreSyncStarted => {
            CliService::section("Pre-Deploy Sync");
            display_destructive_warning();
        },
        DeployEvent::PreSyncDeclined => {
            CliService::warning("Pre-deploy sync skipped by user");
            CliService::warning("Runtime files on the container will be LOST");
        },
        DeployEvent::SyncDryRunFinished(result) => display_dry_run_result(result),
        DeployEvent::SyncErrors(errors) => {
            for err in *errors {
                CliService::error(&format!("Sync error: {}", err));
            }
        },
        DeployEvent::SyncBackupFinished(path) => {
            CliService::success(&format!("Backed up to {}", path.display()));
        },
        DeployEvent::SyncDiff(diff) => display_diff(diff),
        DeployEvent::SyncAlreadyClean => CliService::success("All files are already in sync"),
        DeployEvent::SyncCancelled => {
            CliService::warning("Sync cancelled by user. Backup preserved.");
        },
        DeployEvent::SyncApplied { count } => {
            CliService::success(&format!("Applied {} files from cloud", count));
        },
        DeployEvent::ArtifactsResolved {
            tenant_name,
            binary,
            dockerfile,
        } => {
            CliService::key_value("Tenant", tenant_name);
            CliService::key_value("Binary", &binary.display().to_string());
            CliService::key_value("Dockerfile", &dockerfile.display().to_string());
        },
        DeployEvent::ImageResolved { image } => CliService::key_value("Image", image),
        DeployEvent::BuildFinished => CliService::success("Docker image built"),
        DeployEvent::PushSkipped => CliService::info("Push skipped (--skip-push)"),
        DeployEvent::PushFinished => CliService::success("Image pushed"),
        DeployEvent::SecretsPhaseStarted => CliService::section("Provisioning Secrets"),
        DeployEvent::SecretsFileMissing => {
            CliService::warning("No secrets.json found - skipping secrets sync");
        },
        DeployEvent::SecretsSynced { count } => {
            CliService::success(&format!("Synced {} secrets", count));
        },
        DeployEvent::CredentialsSynced { count } => {
            CliService::success(&format!("Synced {} cloud credentials", count));
        },
        DeployEvent::ProfilePathConfigured => CliService::success("Profile path configured"),
        DeployEvent::Deployed { status, app_url } => {
            CliService::success("Deployed!");
            CliService::key_value("Status", status);
            if let Some(url) = app_url {
                CliService::key_value("URL", url);
            }
        },
        _ => {},
    }
}

fn display_destructive_warning() {
    CliService::warning("DESTRUCTIVE OPERATION");
    CliService::info("  Deploying replaces the running container.");
    CliService::info("  Runtime files (uploads, AI-generated images) not in your local build");
    CliService::info("  will be PERMANENTLY LOST unless synced first.");
    CliService::info("");
    CliService::info("  Database records are preserved.");
    CliService::info("");
}

fn display_diff(diff: &SyncDiffResult) {
    CliService::section("Cloud Sync Diff");

    if diff.added > 0 {
        CliService::info(&format!("  Added ({}):", diff.added));
        for entry in &diff.entries {
            if entry.status == FileDiffStatus::Added {
                CliService::info(&format!("    + {}", entry.path));
            }
        }
    }

    if diff.modified > 0 {
        CliService::info(&format!("  Modified ({}):", diff.modified));
        for entry in &diff.entries {
            if entry.status == FileDiffStatus::Modified {
                CliService::info(&format!("    ~ {}", entry.path));
            }
        }
    }

    if diff.deleted > 0 {
        CliService::info(&format!("  Deleted from cloud ({}):", diff.deleted));
        for entry in &diff.entries {
            if entry.status == FileDiffStatus::Deleted {
                CliService::info(&format!("    - {}", entry.path));
            }
        }
    }

    if diff.unchanged > 0 {
        CliService::info(&format!("  Unchanged ({} files identical)", diff.unchanged));
    }
}

fn display_dry_run_result(result: &SyncOperationResult) {
    CliService::section("Dry Run - Files to Sync");
    match &result.details {
        Some(details) => display_file_list(details),
        None => CliService::info(&format!("Would sync {} items", result.items_skipped)),
    }
}

fn display_file_list(details: &serde_json::Value) {
    let Some(files) = details.get("files").and_then(|f| f.as_array()) else {
        return;
    };

    CliService::info(&format!("Would sync {} files:", files.len()));

    for file in files.iter().take(20) {
        if let Some(path) = file.get("path").and_then(|p| p.as_str()) {
            CliService::info(&format!("  - {}", path));
        }
    }

    if files.len() > 20 {
        CliService::info(&format!("  ... and {} more", files.len() - 20));
    }
}
