//! Profile resolution and the staged bootstrap of CLI runtime state.
//!
//! Resolves the active profile (CLI override, active session, or path), then
//! initialises secrets, credentials, paths, and config in order, and runs
//! startup validation before a command executes.
//!
//! The boot sequence returns one owned [`BootContext`]: only the server runs
//! boot-time model discovery, and every other command loads the services
//! registry as authored.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_cloud::{CloudCredentials, CredentialsBootstrap, SessionStore};
use systemprompt_config::LoadedProfile;
use systemprompt_config::paths::AppPaths;
use systemprompt_identifiers::ProfileName;
use systemprompt_loader::BootContext;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::LogLevel;
use systemprompt_runtime::{
    StartupValidator, display_validation_report, display_validation_warnings,
};

use crate::cli_settings::{CliConfig, OutputFormat, VerbosityLevel};
use crate::env_overrides::EnvOverrides;
use crate::paths::ResolvedPaths;
use crate::shared::{ProfileSource, ResolvedProfile, resolve_profile_path};

pub(super) struct ProfileContext {
    pub loaded: Arc<LoadedProfile>,
    pub source: ProfileSource,
    pub is_cloud: bool,
    pub external_db_access: bool,
    pub env: crate::environment::ExecutionEnvironment,
    pub has_export: bool,
}

pub(super) fn resolve_and_display_profile(
    cli_config: &CliConfig,
    env: &EnvOverrides,
    has_export: bool,
) -> Result<ProfileContext> {
    let resolved = resolve_profile(cli_config.profile_override.as_deref(), env)?;
    let loaded = crate::boot::load_profile(&resolved.path)?;
    let profile = loaded.profile();

    let banner_requested = cli_config.verbosity >= VerbosityLevel::Verbose;
    let banner_warranted = profile.target.is_cloud();
    if cli_config.output_format == OutputFormat::Table
        && cli_config.verbosity != VerbosityLevel::Quiet
        && (banner_requested || banner_warranted)
    {
        let tenant = profile
            .cloud
            .as_ref()
            .and_then(|c| c.tenant_id.as_ref())
            .map(systemprompt_identifiers::TenantId::as_str);
        CliService::profile_banner(&profile.name, profile.target.is_cloud(), tenant);
    }

    let env = crate::environment::ExecutionEnvironment::from_env(env);

    let is_cloud = profile.target.is_cloud();
    let external_db_access = profile.database.external_db_access;
    Ok(ProfileContext {
        loaded,
        source: resolved.source,
        is_cloud,
        external_db_access,
        env,
        has_export,
    })
}

pub(super) fn resolve_profile(
    cli_profile_override: Option<&str>,
    env: &EnvOverrides,
) -> Result<ResolvedProfile> {
    if let Some(profile_input) = cli_profile_override
        && crate::shared::is_path_input(profile_input)
    {
        return crate::shared::resolve_profile_from_path(profile_input)
            .map(|path| ResolvedProfile {
                path,
                source: ProfileSource::Cli,
            })
            .map_err(|e| anyhow::anyhow!("{}", e));
    }

    let session_profile_path = get_active_session_profile_path()?;

    resolve_profile_path(
        cli_profile_override,
        env.profile.as_deref(),
        session_profile_path,
    )
    .context(
        "Profile resolution failed. Use --profile <name> or 'systemprompt admin session switch \
         <profile>'",
    )
}

fn get_active_session_profile_path() -> Result<Option<PathBuf>> {
    let paths = ResolvedPaths::discover();
    let sessions_dir = paths.sessions_dir();

    let Some(store) = SessionStore::load(&sessions_dir)? else {
        return Ok(None);
    };

    if let Some(profile_name) = store.active_profile_name.as_ref()
        && let Some(path) = resolve_profile_path_by_name(&paths, profile_name)
    {
        return Ok(Some(path));
    }

    if let Some(session) = store.active_session_for_profile_discovery()
        && let Some(path) = &session.profile_path
        && path.exists()
    {
        return Ok(Some(path.clone()));
    }

    Ok(None)
}

fn resolve_profile_path_by_name(paths: &ResolvedPaths, name: &ProfileName) -> Option<PathBuf> {
    let profile_dir = paths.profiles_dir().join(name.as_str());
    let config_path = systemprompt_cloud::ProfilePath::Config.resolve(&profile_dir);
    config_path.exists().then_some(config_path)
}

pub(super) fn try_load_log_level(profile_path: &Path) -> Option<LogLevel> {
    Profile::load_from_path(profile_path)
        .ok()
        .map(|profile| profile.runtime.log_level)
}

pub(super) async fn init_credentials_gracefully(
    announce: bool,
) -> Result<Option<CloudCredentials>> {
    match CredentialsBootstrap::load().await {
        Ok(credentials) => Ok(credentials),
        Err(e) if e.is_local_mode_recoverable() => {
            if announce {
                tracing::warn!(
                    error = %e,
                    "Cloud credentials unavailable; continuing in local-only mode. \
                     Cloud commands will require 'systemprompt cloud auth login'."
                );
            } else {
                tracing::debug!(
                    error = %e,
                    "Cloud credentials unavailable; continuing in local-only mode"
                );
            }
            Ok(None)
        },
        Err(e) => Err(anyhow::Error::new(e).context("Credential initialization failed")),
    }
}

pub(super) async fn init_secrets(profile: &Arc<LoadedProfile>) -> Result<BootContext> {
    crate::boot::load_boot(profile).await
}

pub(super) async fn init_paths(
    boot: &BootContext,
    discover_models: bool,
) -> Result<systemprompt_loader::ServicesState> {
    let profile = boot.profile();
    let active_root = boot
        .try_resolve_services_root(env!("CARGO_PKG_VERSION"))
        .await
        .context("Failed to resolve the services bundle sources")?;
    AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        Some(active_root.path.as_path()),
    )
    .context("Failed to build paths")?;
    boot.try_init_config(Some(active_root.path.as_path()))
        .context("Failed to initialize configuration")?;
    let services = if discover_models {
        let secrets = boot.secrets().get();
        systemprompt_loader::ServicesState::load_with_discovery(boot, move |providers| {
            Box::pin(systemprompt_runtime::discover_vertex_models(
                providers, secrets,
            ))
        })
        .await
    } else {
        systemprompt_loader::ServicesState::load(boot)
    }
    .context("Failed to load the services configuration")?;
    Ok(services)
}

pub(super) fn run_validation(boot: &BootContext) -> Result<()> {
    let mut validator = StartupValidator::new(boot.clone())?;
    let report = validator.validate(boot.config()?);

    if report.has_errors() {
        display_validation_report(&report);
        anyhow::bail!("Startup validation failed with errors");
    }

    if report.has_warnings() {
        display_validation_warnings(&report);
    }

    Ok(())
}

pub(super) fn validate_cloud_credentials(
    env: &crate::environment::ExecutionEnvironment,
    credentials: Option<&CloudCredentials>,
    profile: &Profile,
) {
    if env.is_remote_cli {
        return;
    }

    let is_local_profile = profile.is_local_trial();

    match credentials {
        Some(creds) => {
            if creds.is_token_expired() {
                CliService::warning(
                    "Cloud token has expired. Run 'systemprompt cloud auth login' to refresh.",
                );
            }
        },
        None => {
            if is_local_profile {
                tracing::debug!("Cloud credentials not present; running in local-only mode");
            } else {
                CliService::error(
                    "Cloud credentials not found. Run 'systemprompt cloud auth login' to register.",
                );
            }
        },
    }
}
