//! CLI bootstrap and initialization logic.
//!
//! This module handles the initialization sequence for CLI commands,
//! including profile resolution, credentials, secrets, paths, and validation.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use systemprompt_cloud::{CliSession, CredentialsBootstrap, ProjectContext};
use systemprompt_core_files::FilesConfig;
use systemprompt_core_logging::CliService;
use systemprompt_models::{AppPaths, Config, ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::{
    display_validation_report, display_validation_warnings, StartupValidator,
};

use crate::requirements::CommandRequirements;
use crate::shared::resolve_profile_path;

/// Performs the bootstrap sequence based on command requirements.
#[allow(dead_code)]
pub async fn initialize(reqs: &CommandRequirements, skip_validation: bool) -> Result<()> {
    if !reqs.profile {
        return Ok(());
    }

    let profile_path = resolve_profile()?;
    init_profile(&profile_path)?;
    init_credentials().await?;

    if reqs.secrets {
        init_secrets()?;
    }

    if reqs.paths {
        init_paths()?;
        if !skip_validation {
            run_validation()?;
        }
    }

    validate_cloud_credentials();
    Ok(())
}

/// Resolves the profile path from session or environment.
pub fn resolve_profile() -> Result<PathBuf> {
    let project_ctx = ProjectContext::discover();
    let session_path = project_ctx.local_session();
    let session_profile_path = CliSession::try_load_profile_path(&session_path);

    resolve_profile_path(session_profile_path).context(
        "Profile resolution failed. Set SYSTEMPROMPT_PROFILE environment variable or create a \
         profile with 'systemprompt cloud profile create'",
    )
}

/// Initializes the profile from a path.
pub fn init_profile(path: &Path) -> Result<()> {
    ProfileBootstrap::init_from_path(path)
        .with_context(|| format!("Profile initialization failed from: {}", path.display()))?;
    Ok(())
}

/// Initializes cloud credentials.
pub async fn init_credentials() -> Result<()> {
    CredentialsBootstrap::init()
        .await
        .context("Cloud credentials required. Run 'systemprompt cloud login'")?;
    Ok(())
}

/// Initializes secrets from the loaded profile.
pub fn init_secrets() -> Result<()> {
    SecretsBootstrap::init().context("Secrets initialization failed")?;
    Ok(())
}

/// Initializes application paths and configuration.
pub fn init_paths() -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    AppPaths::init(&profile.paths).context("Failed to initialize paths")?;
    Config::try_init().context("Failed to initialize configuration")?;
    FilesConfig::init().context("Failed to initialize files configuration")?;
    Ok(())
}

/// Runs startup validation.
pub fn run_validation() -> Result<()> {
    let mut validator = StartupValidator::new();
    let report = validator.validate(Config::get()?);

    if report.has_errors() {
        display_validation_report(&report);
        #[allow(clippy::exit)]
        std::process::exit(1);
    }

    if report.has_warnings() {
        display_validation_warnings(&report);
    }

    Ok(())
}

/// Validates and warns about cloud credential status.
pub fn validate_cloud_credentials() {
    match CredentialsBootstrap::get() {
        Ok(Some(creds)) => {
            if creds.is_token_expired() {
                CliService::warning(
                    "Cloud token has expired. Run 'systemprompt cloud login' to refresh.",
                );
            }
        },
        Ok(None) => {
            CliService::error(
                "Cloud credentials not found. Run 'systemprompt cloud login' to register.",
            );
        },
        Err(e) => {
            CliService::error(&format!("Cloud credential error: {}", e));
        },
    }
}
