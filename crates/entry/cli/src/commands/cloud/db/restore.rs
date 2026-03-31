use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Command;
use systemprompt_logging::CliService;

use super::{adjust_ssl_mode, ensure_pg_tool, find_pg_restore};
use crate::cli_settings::CliConfig;

pub fn execute(
    profile_name: &str,
    database_url: &str,
    file: &str,
    skip_confirm: bool,
    config: &CliConfig,
) -> Result<()> {
    let file_path = PathBuf::from(file);
    if !file_path.exists() {
        bail!("Backup file not found: {}", file);
    }

    let is_custom_or_dir = std::path::Path::new(file)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dump"))
        || file_path.is_dir();

    if is_custom_or_dir {
        find_pg_restore()?;
    } else {
        ensure_pg_tool("psql")?;
    }

    CliService::section("Cloud Database Restore");
    CliService::key_value("Profile", profile_name);
    CliService::key_value("File", file);
    CliService::warning("This will overwrite data in the cloud database!");

    if !skip_confirm {
        if !config.is_interactive() {
            bail!("Restore requires -y flag in non-interactive mode");
        }

        let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(format!(
                "Restore backup to cloud database for profile '{}'?",
                profile_name
            ))
            .default(false)
            .interact()?;

        if !confirm {
            CliService::info("Cancelled");
            return Ok(());
        }
    }

    let adjusted_url = adjust_ssl_mode(database_url);
    let spinner = CliService::spinner("Restoring database...");

    let result = if is_custom_or_dir {
        let pg_restore = find_pg_restore()?;
        Command::new(pg_restore)
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("--clean")
            .arg("--if-exists")
            .arg("-d")
            .arg(&adjusted_url)
            .arg(file)
            .output()
            .context("Failed to execute pg_restore")?
    } else {
        Command::new("psql")
            .arg(&adjusted_url)
            .arg("-f")
            .arg(file)
            .output()
            .context("Failed to execute psql")?
    };

    spinner.finish_and_clear();

    if result.status.success() {
        CliService::success("Database restored successfully");
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        if stderr.contains("ERROR") {
            bail!("Restore failed:\n{}", stderr);
        }
        CliService::warning("Restore completed with warnings:");
        CliService::info(&stderr.chars().take(500).collect::<String>());
    }

    Ok(())
}
