use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError, SessionStore};
use systemprompt_files::FilesConfig;
use systemprompt_logging::CliService;
use systemprompt_models::profile::LogLevel;
use systemprompt_models::{AppPaths, Config, Profile, ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::{
    display_validation_report, display_validation_warnings, StartupValidator,
};

use crate::cli_settings::{CliConfig, OutputFormat, VerbosityLevel};
use crate::paths::ResolvedPaths;
use crate::shared::resolve_profile_path;

pub struct ProfileContext {
    pub profile_name: String,
    pub is_cloud: bool,
    pub external_db_access: bool,
    pub env: crate::environment::ExecutionEnvironment,
    pub has_export: bool,
}

pub fn resolve_and_display_profile(
    cli_config: &CliConfig,
    has_export: bool,
) -> Result<ProfileContext> {
    let profile_path = resolve_profile(cli_config.profile_override.as_deref())?;
    init_profile(&profile_path)?;

    let profile = ProfileBootstrap::get()?;

    if cli_config.output_format == OutputFormat::Table
        && cli_config.verbosity != VerbosityLevel::Quiet
    {
        let tenant = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_deref());
        CliService::profile_banner(&profile.name, profile.target.is_cloud(), tenant);
    }

    let env = crate::environment::ExecutionEnvironment::detect();

    Ok(ProfileContext {
        profile_name: profile.name.clone(),
        is_cloud: profile.target.is_cloud(),
        external_db_access: profile.database.external_db_access,
        env,
        has_export,
    })
}

pub fn resolve_profile(cli_profile_override: Option<&str>) -> Result<PathBuf> {
    if let Some(profile_input) = cli_profile_override {
        if crate::shared::is_path_input(profile_input) {
            return crate::shared::resolve_profile_from_path(profile_input)
                .map_err(|e| anyhow::anyhow!("{}", e));
        }
    }

    let session_profile_path = get_active_session_profile_path();

    resolve_profile_path(cli_profile_override, session_profile_path).context(
        "Profile resolution failed. Use --profile <name> or 'systemprompt admin session switch \
         <profile>'",
    )
}

fn get_active_session_profile_path() -> Option<PathBuf> {
    let paths = ResolvedPaths::discover();
    let sessions_dir = paths.sessions_dir().ok()?;

    let store = SessionStore::load(&sessions_dir)?;

    if let Some(profile_name) = store.active_profile_name.as_deref() {
        if let Some(path) = resolve_profile_path_by_name(&paths, profile_name) {
            return Some(path);
        }
    }

    if let Some(session) = store.active_session() {
        if let Some(path) = &session.profile_path {
            if path.exists() {
                return Some(path.clone());
            }
        }
    }

    None
}

fn resolve_profile_path_by_name(paths: &ResolvedPaths, name: &str) -> Option<PathBuf> {
    let profile_dir = paths.profiles_dir().join(name);
    let config_path = systemprompt_cloud::ProfilePath::Config.resolve(&profile_dir);
    config_path.exists().then_some(config_path)
}

pub fn init_profile(path: &Path) -> Result<()> {
    ProfileBootstrap::init_from_path(path)
        .with_context(|| format!("Profile initialization failed from: {}", path.display()))?;
    Ok(())
}

pub fn try_load_log_level(profile_path: &Path) -> Option<LogLevel> {
    let content = std::fs::read_to_string(profile_path).ok()?;
    let profile: Profile = serde_yaml::from_str(&content).ok()?;
    Some(profile.runtime.log_level)
}

pub async fn init_credentials() -> Result<()> {
    CredentialsBootstrap::init().await?;
    Ok(())
}

pub async fn init_credentials_gracefully() -> Result<()> {
    match init_credentials().await {
        Ok(()) => Ok(()),
        Err(e) => {
            let is_file_not_found = e
                .downcast_ref::<CredentialsBootstrapError>()
                .is_some_and(|ce| matches!(ce, CredentialsBootstrapError::FileNotFound { .. }));

            if is_file_not_found {
                tracing::debug!(error = %e, "Credentials file not found, continuing in local-only mode");
                Ok(())
            } else {
                Err(e.context("Credential initialization failed"))
            }
        },
    }
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
