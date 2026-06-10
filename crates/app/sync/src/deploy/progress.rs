//! Progress seam between the deploy pipeline and its presentation layer.
//!
//! The orchestrator emits a [`DeployEvent`] at every step boundary and asks
//! the caller to resolve each [`DeployPrompt`]; implementations decide how
//! (or whether) to render and answer them. Events carry borrowed data, so no
//! rendering state lives in the pipeline.

use std::path::Path;

use crate::error::SyncResult;
use crate::files::SyncDiffResult;
use crate::result::SyncOperationResult;

pub trait DeployProgress: Send + Sync {
    fn event(&self, event: &DeployEvent<'_>);
    fn confirm(&self, prompt: &DeployPrompt) -> SyncResult<bool>;
}

#[derive(Debug, Clone, Copy)]
pub enum DeployEvent<'a> {
    PreSyncSkippedByFlag,
    PreSyncStarted,
    PreSyncDeclined,
    SyncDryRunStarted,
    SyncDryRunFinished(&'a SyncOperationResult),
    SyncErrors(&'a [String]),
    SyncDownloadStarted,
    SyncDownloadFinished,
    SyncBackupStarted,
    SyncBackupFinished(&'a Path),
    SyncDiff(&'a SyncDiffResult),
    SyncAlreadyClean,
    SyncCancelled,
    SyncApplied {
        count: usize,
    },
    ArtifactsResolved {
        tenant_name: &'a str,
        binary: &'a Path,
        dockerfile: &'a Path,
    },
    RegistryAuthStarted,
    RegistryAuthFinished,
    ImageResolved {
        image: &'a str,
    },
    BuildStarted,
    BuildFinished,
    PushSkipped,
    PushStarted,
    PushFinished,
    SecretsPhaseStarted,
    SecretsFileMissing,
    SecretsSyncStarted,
    SecretsSynced {
        count: usize,
    },
    CredentialsSyncStarted,
    CredentialsSynced {
        count: usize,
    },
    ProfilePathConfigured,
    DeployStarted,
    Deployed {
        status: &'a str,
        app_url: Option<&'a str>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum DeployPrompt {
    PreSync,
    ApplyChanges { count: usize },
}
