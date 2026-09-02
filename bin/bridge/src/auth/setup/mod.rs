//! PAT validation and on-disk persistence for first-run auth setup.
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

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{0}")]
    Token(String),
    #[error("{0}")]
    Path(String),
    #[error("{0}")]
    Io(String),
    #[error("task join: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("gateway: {0}")]
    Gateway(#[from] crate::gateway::GatewayError),
    // Why: the user (or a superseding request) stopped this before it could
    // conclude. It is not a failure and must never be reported as one.
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
    let base = crate::basedirs::config_dir().ok_or_else(|| {
        SetupError::Path("no OS config directory available on this platform".to_owned())
    })?;
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
    write_pat_file(&paths.pat_file, token)?;
    write_config_file(&paths.config_file, &paths.pat_file, gateway_url)?;
    invalidate_cached_token()?;
    tracing::info!(config_file = %paths.config_file.display(), "login: PAT and config written");
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn set_gateway_url(gateway_url: &str) -> Result<PathLayout, SetupError> {
    let trimmed = gateway_url.trim();
    if trimmed.is_empty() {
        return Err(SetupError::Path("gateway_url is empty".into()));
    }
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    write_config_file(&paths.config_file, &paths.pat_file, Some(trimmed))?;
    invalidate_cached_token()?;
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn logout() -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    remove_if_exists(&paths.pat_file)?;
    remove_managed_mcp_fragment()?;
    remove_sync_state()?;
    if let Err(e) = crate::auth::cache::clear() {
        return Err(SetupError::Io(format!("clear token cache: {e}")));
    }
    if let Err(e) = crate::auth::plugin_oauth::delete_creds() {
        return Err(SetupError::Io(format!("clear oauth client creds: {e}")));
    }
    if paths.config_file.exists() {
        match fs::read_to_string(&paths.config_file) {
            Ok(existing) => {
                let stripped = strip_credential_sections(&existing)?;
                if stripped.trim().is_empty() {
                    remove_if_exists(&paths.config_file)?;
                } else {
                    atomic_write(&paths.config_file, stripped.as_bytes(), false)?;
                }
            },
            Err(e) => return Err(SetupError::Io(format!("read config: {e}"))),
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
    if let Err(e) = crate::auth::cache::clear() {
        return Err(SetupError::Io(format!("clear token cache: {e}")));
    }
    let oauth_creds_removed = crate::auth::plugin_oauth::creds_path().is_some_and(|p| p.exists());
    if let Err(e) = crate::auth::plugin_oauth::delete_creds() {
        return Err(SetupError::Io(format!("clear oauth client creds: {e}")));
    }
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
    let gateway = resolve_gateway(&paths.config_file, gateway_url);
    merge_config_file(&paths.config_file, &gateway, "session", |doc| {
        crate::config::write::set(doc, &["session", "enabled"], true);
    })?;
    invalidate_cached_token()?;
    tracing::info!(config_file = %paths.config_file.display(), "session setup: config written");
    Ok(paths)
}

fn validate_token(token: &str) -> Result<(), SetupError> {
    let trimmed = token.trim();
    if !trimmed.starts_with(PAT_PREFIX) {
        return Err(SetupError::Token(format!(
            "token must start with `{PAT_PREFIX}`"
        )));
    }
    if !trimmed.contains('.') {
        return Err(SetupError::Token(
            "token must contain a `.` separator (sp-live-<prefix>.<secret>)".into(),
        ));
    }
    if trimmed.len() < 40 {
        return Err(SetupError::Token(
            "token looks too short — did the copy get truncated?".into(),
        ));
    }
    Ok(())
}

fn invalidate_cached_token() -> Result<(), SetupError> {
    crate::auth::cache::clear().map_err(|e| SetupError::Io(format!("clear token cache: {e}")))
}
