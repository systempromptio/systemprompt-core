use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_cloud::{CloudApiClient, CloudPath, ProfilePath, TenantStore, get_cloud_paths};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_sync::{FileDiffStatus, SyncDiffResult, SyncOperationResult};

use crate::cli_settings::CliConfig;
use crate::commands::cloud::tenant::get_credentials;
use crate::interactive::confirm_optional;

pub fn display_diff(diff: &SyncDiffResult) {
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

pub fn display_destructive_warning() {
    CliService::warning("DESTRUCTIVE OPERATION");
    CliService::info("  Deploying replaces the running container.");
    CliService::info("  Runtime files (uploads, AI-generated images) not in your local build");
    CliService::info("  will be PERMANENTLY LOST unless synced first.");
    CliService::info("");
    CliService::info("  Database records are preserved.");
    CliService::info("");
}

pub fn display_dry_run_result(result: &SyncOperationResult) {
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

pub async fn setup_sync_token(
    tenant_id: &TenantId,
    yes: bool,
    cli_config: &CliConfig,
    profile_path: &Path,
    tenant_store: &mut TenantStore,
) -> Result<Option<String>> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let creds = get_credentials()?;

    CliService::section("Sync Token Setup");
    CliService::warning("Sync token not configured in profile secrets");

    let stored_token = tenant_store
        .find_tenant(tenant_id.as_str())
        .and_then(|t| t.sync_token.clone());

    let token = if let Some(token) = stored_token {
        CliService::info("Found existing sync token in local tenant store");
        token
    } else {
        CliService::info("No sync token found - generating a new one...");

        if !yes {
            let should_generate = confirm_optional(
                "Generate a new sync token for file synchronization?",
                true,
                cli_config,
            )?;
            if !should_generate {
                anyhow::bail!(
                    "Sync token required for pre-deploy sync.\nUse --no-sync to skip sync \
                     explicitly."
                );
            }
        }

        let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;
        let spinner = CliService::spinner("Generating sync token...");
        let response = client.rotate_sync_token(tenant_id.as_str()).await?;
        spinner.finish_and_clear();

        let token = response.sync_token;
        CliService::success("Sync token generated");

        if let Some(tenant) = tenant_store.tenants.iter_mut().find(|t| t.id == tenant_id.as_str()) {
            tenant.sync_token = Some(token.clone());
        }
        tenant_store.save_to_path(&tenants_path)?;

        token
    };

    save_sync_token_to_secrets(profile_path, &token)?;
    CliService::success("Sync token saved to profile secrets");

    Ok(Some(token))
}

fn save_sync_token_to_secrets(profile_path: &Path, token: &str) -> Result<()> {
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path"))?;
    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        anyhow::bail!(
            "Secrets file not found: {}\nRun 'systemprompt cloud profile create' first.",
            secrets_path.display()
        );
    }

    let content = std::fs::read_to_string(&secrets_path)
        .with_context(|| format!("Failed to read {}", secrets_path.display()))?;

    let mut secrets: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse secrets.json")?;

    secrets["sync_token"] = serde_json::Value::String(token.to_string());

    let updated = serde_json::to_string_pretty(&secrets).context("Failed to serialize secrets")?;

    std::fs::write(&secrets_path, updated)
        .with_context(|| format!("Failed to write {}", secrets_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&secrets_path, permissions)
            .with_context(|| format!("Failed to set permissions on {}", secrets_path.display()))?;
    }

    Ok(())
}
