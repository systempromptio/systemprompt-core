use systemprompt_core_logging::CliService;
use systemprompt_sync::{ContentDiffEntry, LocalSyncResult};

pub fn display_diff_summary(diffs: &[ContentDiffEntry]) {
    for entry in diffs {
        CliService::section(&format!("Source: {}", entry.name));
        CliService::info(&format!("{} unchanged", entry.diff.unchanged));

        if !entry.diff.added.is_empty() {
            CliService::info(&format!(
                "+ {} (on disk, not in DB)",
                entry.diff.added.len()
            ));
            for item in &entry.diff.added {
                CliService::info(&format!("    + {}", item.slug));
            }
        }

        if !entry.diff.removed.is_empty() {
            CliService::info(&format!(
                "- {} (in DB, not on disk)",
                entry.diff.removed.len()
            ));
            for item in &entry.diff.removed {
                CliService::info(&format!("    - {}", item.slug));
            }
        }

        if !entry.diff.modified.is_empty() {
            CliService::info(&format!("~ {} (modified)", entry.diff.modified.len()));
            for item in &entry.diff.modified {
                CliService::info(&format!("    ~ {}", item.slug));
            }
        }
    }
}

pub fn display_sync_result(result: &LocalSyncResult) {
    CliService::section("Sync Complete");
    CliService::key_value("Direction", &result.direction);
    CliService::key_value("Synced", &result.items_synced.to_string());
    CliService::key_value("Deleted", &result.items_deleted.to_string());
    CliService::key_value("Skipped", &result.items_skipped.to_string());

    if !result.errors.is_empty() {
        CliService::warning(&format!("Errors ({})", result.errors.len()));
        for error in &result.errors {
            CliService::error(&format!("    {}", error));
        }
    }
}
