//! PAT validation and on-disk persistence for first-run auth setup.
//!
//! When a login or gateway change moves the configured gateway to a
//! different origin, the previous gateway's synced state (the managed MCP
//! fragment and the last-sync sentinel) is removed. Both are gateway-stamped
//! and would be ignored anyway; removing them keeps a switch from leaving
//! another gateway's servers on disk for diagnostics to misread.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config_file;
mod files;

use config_file::{
    merge_config_file, resolve_gateway, strip_credential_sections, write_config_file,
};
use files::{
    atomic_write, ensure_dir, remove_if_exists, remove_managed_mcp_fragment, remove_sync_state,
    write_pat_file,
};
use std::fs;
use std::path::PathBuf;

const PAT_PREFIX: &str = "sp-live-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRejection {
    Prefix,
    Separator,
    TooShort,
}

impl std::fmt::Display for TokenRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prefix => write!(f, "token must start with `{PAT_PREFIX}`"),
            Self::Separator => {
                f.write_str("token must contain a `.` separator (sp-live-<prefix>.<secret>)")
            },
            Self::TooShort => f.write_str("token looks too short — did the copy get truncated?"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{0}")]
    Token(TokenRejection),
    #[error("gateway_url is empty")]
    EmptyGateway,
    #[error("cannot resolve {0}")]
    Unresolvable(&'static str),
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },
    #[error("{path}: gateway_url must be a nonempty string")]
    GatewayNotString { path: PathBuf },
    #[error(transparent)]
    ConfigWrite(#[from] crate::config::ConfigWriteError),
    #[error(transparent)]
    ConfigRead(#[from] crate::config::ConfigReadError),
    #[error("token cache: {0}")]
    Cache(#[source] std::io::Error),
    #[error("credential binding: {0}")]
    Binding(#[source] std::io::Error),
    #[error(transparent)]
    PluginOAuth(#[from] crate::auth::plugin_oauth::PluginOAuthError),
    #[error("device link: {0}")]
    DeviceLink(#[source] crate::auth::providers::AuthError),
    #[error("gateway changed during session sign-in")]
    GatewayMoved,
    #[error("task join: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("gateway: {0}")]
    Gateway(#[from] crate::gateway::GatewayError),
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug)]
pub struct PathLayout {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub pat_file: PathBuf,
}

pub fn resolve_paths() -> Result<PathLayout, SetupError> {
    let base =
        crate::basedirs::config_dir().ok_or(SetupError::Unresolvable("OS config directory"))?;
    let brand = crate::brand::brand();
    let config_dir = base.join(brand.config_dir);
    let config_file = config_dir.join(brand.config_file);
    let pat_file = config_dir.join(brand.pat_file);
    Ok(PathLayout {
        config_dir,
        config_file,
        pat_file,
    })
}

#[tracing::instrument(level = "debug", skip(token), fields(has_gateway = gateway_url.is_some()))]
pub fn login(token: &str, gateway_url: Option<&str>) -> Result<PathLayout, SetupError> {
    validate_token(token)?;
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    let previous = configured_gateway()?;
    write_pat_file(&paths.pat_file, token)?;
    write_config_file(&paths.config_file, &paths.pat_file, gateway_url)?;
    invalidate_cached_token()?;
    forget_gateway_state_if_moved(&previous)?;
    tracing::info!(config_file = %paths.config_file.display(), "login: PAT and config written");
    Ok(paths)
}

fn configured_gateway() -> Result<systemprompt_identifiers::ValidatedUrl, SetupError> {
    let cfg = crate::config::load()?;
    Ok(crate::config::gateway_url_or_default(&cfg))
}

fn forget_gateway_state_if_moved(
    previous: &systemprompt_identifiers::ValidatedUrl,
) -> Result<(), SetupError> {
    let current = configured_gateway()?;
    if crate::mcp_registry::same_origin(previous, &current) {
        return Ok(());
    }
    tracing::info!(
        previous = %previous,
        current = %current,
        "gateway changed; forgetting the previous gateway's synced state"
    );
    forget_gateway_state()
}

pub fn forget_gateway_state() -> Result<(), SetupError> {
    remove_managed_mcp_fragment()?;
    remove_sync_state()
}

#[tracing::instrument(level = "debug")]
pub fn set_gateway_url(gateway_url: &str) -> Result<PathLayout, SetupError> {
    let trimmed = gateway_url.trim();
    if trimmed.is_empty() {
        return Err(SetupError::EmptyGateway);
    }
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    let previous = configured_gateway()?;
    write_config_file(&paths.config_file, &paths.pat_file, Some(trimmed))?;
    invalidate_cached_token()?;
    forget_gateway_state_if_moved(&previous)?;
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn logout() -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    remove_if_exists(&paths.pat_file)?;
    remove_managed_mcp_fragment()?;
    remove_sync_state()?;
    crate::auth::cache::clear().map_err(SetupError::Cache)?;
    crate::auth::plugin_oauth::delete_creds()?;
    if paths.config_file.exists() {
        let existing = fs::read_to_string(&paths.config_file).map_err(|source| SetupError::Io {
            action: "read",
            path: paths.config_file.clone(),
            source,
        })?;
        let stripped = strip_credential_sections(&paths.config_file, &existing)?;
        if stripped.trim().is_empty() {
            remove_if_exists(&paths.config_file)?;
        } else {
            atomic_write(&paths.config_file, stripped.as_bytes(), true)?;
        }
    }
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn clean() -> Result<CleanReport, SetupError> {
    let paths = resolve_paths()?;
    let pat_removed = paths.pat_file.exists();
    remove_if_exists(&paths.pat_file)?;
    let config_removed = paths.config_file.exists();
    remove_if_exists(&paths.config_file)?;
    remove_managed_mcp_fragment()?;
    remove_sync_state()?;
    if let Some(dir) = crate::config::paths::bridge_metadata_dir() {
        remove_if_exists(&dir.join(crate::config::paths::FIRST_RUN_SENTINEL))?;
        remove_if_exists(&dir.join(crate::config::paths::ONBOARDED_SENTINEL))?;
    }

    crate::auth::cache::clear().map_err(SetupError::Cache)?;
    let oauth_creds_removed = crate::auth::plugin_oauth::creds_path().is_some_and(|p| p.exists());
    crate::auth::plugin_oauth::delete_creds()?;
    Ok(CleanReport {
        paths,
        pat_removed,
        config_removed,
        oauth_creds_removed,
    })
}

#[derive(Debug)]
pub struct CleanReport {
    pub paths: PathLayout,
    pub pat_removed: bool,
    pub config_removed: bool,
    pub oauth_creds_removed: bool,
}

pub fn status() -> Result<StatusReport, SetupError> {
    let paths = resolve_paths()?;
    let config_present = paths.config_file.exists();
    let pat_present = paths.pat_file.exists();
    let oauth_creds_path = crate::auth::plugin_oauth::creds_path();
    let oauth_creds_present = oauth_creds_path.as_ref().is_some_and(|p| p.exists());
    Ok(StatusReport {
        paths,
        config_present,
        pat_present,
        oauth_creds_path,
        oauth_creds_present,
    })
}

#[derive(Debug)]
pub struct StatusReport {
    pub paths: PathLayout,
    pub config_present: bool,
    pub pat_present: bool,
    pub oauth_creds_path: Option<PathBuf>,
    pub oauth_creds_present: bool,
}

pub fn session_setup(gateway_url: Option<&str>) -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    let gateway = resolve_gateway(&paths.config_file, gateway_url)?;
    merge_config_file(&paths.config_file, &gateway, "session", |doc| {
        crate::config::write::set(doc, &["session", "enabled"], true)?;
        crate::config::write::set(
            doc,
            &["session", "generation"],
            uuid::Uuid::new_v4().to_string(),
        )
    })?;
    invalidate_cached_token()?;
    tracing::info!(config_file = %paths.config_file.display(), "session setup: config written");
    Ok(paths)
}

fn validate_token(token: &str) -> Result<(), SetupError> {
    let trimmed = token.trim();
    if !trimmed.starts_with(PAT_PREFIX) {
        return Err(SetupError::Token(TokenRejection::Prefix));
    }
    if !trimmed.contains('.') {
        return Err(SetupError::Token(TokenRejection::Separator));
    }
    if trimmed.len() < 40 {
        return Err(SetupError::Token(TokenRejection::TooShort));
    }
    Ok(())
}

fn invalidate_cached_token() -> Result<(), SetupError> {
    crate::auth::cache::clear().map_err(SetupError::Cache)
}
