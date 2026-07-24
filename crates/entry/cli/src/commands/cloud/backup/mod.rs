//! `cloud backup` command: download the tenant's runtime `services/` tree.
//!
//! Deploys are stateless container rebuilds — runtime files created inside
//! the live container (uploads, AI-generated images) do not survive them.
//! This command downloads that tree as a one-shot backup into a standalone
//! directory. It never writes into the project's own `services/` directory
//! and is deliberately not part of the deploy flow.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod extract;

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use systemprompt_logging::CliService;

use client::BackupClient;
use extract::extract_tarball;

use super::deploy::{resolve_deploy_target, resolve_profile};
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;

pub(super) struct BackupArgs {
    pub profile_name: Option<String>,
    pub output: Option<PathBuf>,
    pub list: bool,
}

pub(super) async fn execute(
    args: BackupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    CliService::section("systemprompt.io Cloud Backup");

    let (profile, _profile_path) = resolve_profile(prompter, args.profile_name.as_deref(), config)?;
    if profile.target != systemprompt_models::ProfileType::Cloud {
        bail!("Cannot back up a local profile. Select a cloud profile with --profile <name>.");
    }

    let target = resolve_deploy_target(&profile)?;
    let hostname = target.hostname.ok_or_else(|| {
        anyhow!(
            "Tenant {} has no hostname. Run 'systemprompt cloud login' to refresh tenants.",
            target.tenant_id
        )
    })?;

    let spinner = CliService::spinner("Authenticating with tenant deployment...");
    let client = BackupClient::connect(&hostname, target.creds.api_token.as_str()).await?;
    spinner.finish_and_clear();

    if args.list {
        let manifest = client.fetch_manifest().await?;
        CliService::info(&format!("{} files on {}", manifest.files.len(), hostname));
        for file in &manifest.files {
            CliService::info(&format!("  {} ({} bytes)", file.path, file.size));
        }
        return Ok(());
    }

    let output = args.output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "systemprompt-backup-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ))
    });
    std::fs::create_dir_all(&output)?;

    let spinner = CliService::spinner("Downloading services tree...");
    let bundle = client.download_bundle().await?;
    spinner.finish_and_clear();

    let count = extract_tarball(&bundle, &output)?;
    CliService::success(&format!(
        "Backed up {} files from {} to {}",
        count,
        hostname,
        output.display()
    ));

    Ok(())
}
