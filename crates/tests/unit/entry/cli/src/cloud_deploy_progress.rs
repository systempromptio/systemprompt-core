//! Unit tests for `cloud::deploy::progress`.
//!
//! [`spinner_message`] is a pure event→label mapping. [`CliDeployProgress`] is
//! driven over every [`DeployEvent`] variant to prove the render path never
//! panics, and its confirm seam is checked in the non-interactive mode where it
//! must return the prompt default without prompting.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::cloud::deploy::progress::{CliDeployProgress, spinner_message};
use systemprompt_sync::deploy::{DeployEvent, DeployProgress, DeployPrompt};
use systemprompt_sync::{
    FileDiffStatus, SyncDiffEntry, SyncDiffResult, SyncOpState, SyncOperationResult,
};

fn op_result() -> SyncOperationResult {
    SyncOperationResult {
        operation: "sync".to_owned(),
        success: true,
        items_synced: 0,
        items_skipped: 3,
        errors: vec![],
        details: Some(serde_json::json!({
            "files": [{ "path": "a.txt" }, { "path": "b.txt" }]
        })),
        state: SyncOpState::Completed,
    }
}

fn diff_result() -> SyncDiffResult {
    SyncDiffResult {
        entries: vec![
            SyncDiffEntry {
                path: "added.txt".to_owned(),
                status: FileDiffStatus::Added,
                size: 1,
            },
            SyncDiffEntry {
                path: "mod.txt".to_owned(),
                status: FileDiffStatus::Modified,
                size: 2,
            },
            SyncDiffEntry {
                path: "gone.txt".to_owned(),
                status: FileDiffStatus::Deleted,
                size: 3,
            },
            SyncDiffEntry {
                path: "same.txt".to_owned(),
                status: FileDiffStatus::Unchanged,
                size: 4,
            },
        ],
        added: 1,
        modified: 1,
        deleted: 1,
        unchanged: 1,
    }
}

#[test]
fn spinner_message_maps_long_running_starts() {
    assert_eq!(
        spinner_message(&DeployEvent::BuildStarted),
        Some("Building Docker image...")
    );
    assert_eq!(
        spinner_message(&DeployEvent::PushStarted),
        Some("Pushing to registry...")
    );
    assert_eq!(
        spinner_message(&DeployEvent::DeployStarted),
        Some("Deploying...")
    );
}

#[test]
fn spinner_message_is_none_for_terminal_events() {
    assert_eq!(spinner_message(&DeployEvent::BuildFinished), None);
    assert_eq!(spinner_message(&DeployEvent::PushFinished), None);
    assert_eq!(spinner_message(&DeployEvent::SyncAlreadyClean), None);
}

#[test]
fn event_renders_every_variant_without_panic() {
    let op = op_result();
    let diff = diff_result();
    let errors = vec!["boom".to_owned()];
    let backup = Path::new("/tmp/backup");
    let binary = Path::new("/tmp/bin");
    let dockerfile = Path::new("/tmp/Dockerfile");

    let events = vec![
        DeployEvent::PreSyncSkippedByFlag,
        DeployEvent::PreSyncStarted,
        DeployEvent::PreSyncDeclined,
        DeployEvent::SyncDryRunStarted,
        DeployEvent::SyncDryRunFinished(&op),
        DeployEvent::SyncErrors(&errors),
        DeployEvent::SyncDownloadStarted,
        DeployEvent::SyncDownloadFinished,
        DeployEvent::SyncBackupStarted,
        DeployEvent::SyncBackupFinished(backup),
        DeployEvent::SyncDiff(&diff),
        DeployEvent::SyncAlreadyClean,
        DeployEvent::SyncCancelled,
        DeployEvent::SyncApplied { count: 2 },
        DeployEvent::ArtifactsResolved {
            tenant_name: "acme",
            binary,
            dockerfile,
        },
        DeployEvent::RegistryAuthStarted,
        DeployEvent::RegistryAuthFinished,
        DeployEvent::ImageResolved { image: "img:1" },
        DeployEvent::BuildStarted,
        DeployEvent::BuildFinished,
        DeployEvent::PushSkipped,
        DeployEvent::PushStarted,
        DeployEvent::PushFinished,
        DeployEvent::SecretsPhaseStarted,
        DeployEvent::SecretsFileMissing,
        DeployEvent::SecretsSyncStarted,
        DeployEvent::SecretsSynced { count: 4 },
        DeployEvent::CredentialsSyncStarted,
        DeployEvent::CredentialsSynced { count: 5 },
        DeployEvent::ProfilePathConfigured,
        DeployEvent::DeployStarted,
        DeployEvent::Deployed {
            status: "running",
            app_url: Some("https://example.com"),
        },
        DeployEvent::Deployed {
            status: "running",
            app_url: None,
        },
    ];

    let progress = CliDeployProgress::non_interactive();
    for event in &events {
        progress.event(event);
    }
}

#[test]
fn non_interactive_confirm_returns_prompt_default() {
    let progress = CliDeployProgress::non_interactive();
    assert!(progress.confirm(&DeployPrompt::PreSync).expect("no prompt"));
    assert!(
        progress
            .confirm(&DeployPrompt::ApplyChanges { count: 1 })
            .expect("no prompt")
    );
}
