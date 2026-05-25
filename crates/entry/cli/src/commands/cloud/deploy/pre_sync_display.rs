use anyhow::Result;
use systemprompt_logging::CliService;
use systemprompt_sync::{FileDiffStatus, SyncDiffResult, SyncOperationResult};

use super::pre_sync::PreSyncResult;

pub(super) fn display_diff(diff: &SyncDiffResult) {
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

pub(super) fn display_destructive_warning() {
    CliService::warning("DESTRUCTIVE OPERATION");
    CliService::info("  Deploying replaces the running container.");
    CliService::info("  Runtime files (uploads, AI-generated images) not in your local build");
    CliService::info("  will be PERMANENTLY LOST unless synced first.");
    CliService::info("");
    CliService::info("  Database records are preserved.");
    CliService::info("");
}

pub(super) fn handle_sync_result(
    result: systemprompt_sync::SyncResult<SyncOperationResult>,
    dry_run: bool,
) -> Result<PreSyncResult> {
    match result {
        Ok(op) if op.success && dry_run => {
            display_dry_run_result(&op);
            Ok(PreSyncResult::dry_run())
        },
        Ok(op) if op.success => {
            CliService::success(&format!("Synced {} files from cloud", op.items_synced));
            Ok(PreSyncResult::success())
        },
        Ok(op) => {
            for err in &op.errors {
                CliService::error(&format!("Sync error: {}", err));
            }
            anyhow::bail!("Pre-deploy sync failed. Use --no-sync to skip (WARNING: may lose data).")
        },
        Err(e) => {
            CliService::error(&format!("Sync error: {}", e));
            anyhow::bail!("Pre-deploy sync failed. Use --no-sync to skip (WARNING: may lose data).")
        },
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
