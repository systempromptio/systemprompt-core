use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use systemprompt_cloud::{CredentialsBootstrap, SessionStore};
use systemprompt_files::FilesConfig;
use systemprompt_logging::CliService;
use systemprompt_models::{AppPaths, Config, ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::{
    display_validation_report, display_validation_warnings, StartupValidator,
};

use crate::paths::ResolvedPaths;
use crate::shared::resolve_profile_path;

pub fn resolve_profile(cli_profile_override: Option<&str>) -> Result<PathBuf> {
    let session_profile_path = get_active_session_profile_path();

    resolve_profile_path(cli_profile_override, session_profile_path).context(
        "Profile resolution failed. Use --profile <name> or 'systemprompt admin session switch \
         <profile>'",
    )
}

fn get_active_session_profile_path() -> Option<PathBuf> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir().ok()?;

    SessionStore::load(&sessions_dir)?
        .active_session()
        .and_then(|s| s.profile_path.clone())
}

pub fn init_profile(path: &Path) -> Result<()> {
    ProfileBootstrap::init_from_path(path)
        .with_context(|| format!("Profile initialization failed from: {}", path.display()))?;
    Ok(())
}

pub async fn init_credentials() -> Result<()> {
    CredentialsBootstrap::init()
        .await
        .context("Cloud credentials required. Run 'systemprompt cloud login'")?;
    Ok(())
}

pub fn init_secrets() -> Result<()> {
    SecretsBootstrap::init().context("Secrets initialization failed")?;
    Ok(())
}

pub fn init_paths() -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    AppPaths::init(&profile.paths).context("Failed to initialize paths")?;
    Config::try_init().context("Failed to initialize configuration")?;
    FilesConfig::init().context("Failed to initialize files configuration")?;
    Ok(())
}

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

pub fn validate_cloud_credentials(env: &crate::environment::ExecutionEnvironment) {
    if env.is_remote_cli {
        return;
    }

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
