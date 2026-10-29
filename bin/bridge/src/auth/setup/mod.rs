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
mod error;
mod files;

use config_file::{
    merge_config_file, resolve_gateway, strip_credential_sections, write_config_file,
};
pub use error::{SetupError, TokenRejection};
use files::{
    atomic_write, ensure_dir, remove_if_exists, remove_managed_mcp_fragment, remove_sync_state,
};

use crate::config::gateways::credential::{self, PatStore};
use crate::config::gateways::{self, ActiveProfile};
use crate::config::write;
use std::fs;
use std::path::PathBuf;

const PAT_PREFIX: &str = "sp-live-";

#[derive(Debug)]
pub struct PathLayout {
    pub profile: ActiveProfile,
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub pat_file: PathBuf,
    pub pat_store: Option<PatStore>,
}

impl PathLayout {
    #[must_use]
    pub fn pat_location(&self) -> String {
        match self.pat_store {
            Some(PatStore::Keyring) => format!(
                "OS credential store ({} / pat:{})",
                crate::brand::brand().keyring_service,
                self.profile.id
            ),
            Some(PatStore::File) => format!("{} (0600)", self.pat_file.display()),
            None => "none".to_owned(),
        }
    }
}

pub fn resolve_paths() -> Result<PathLayout, SetupError> {
    let profile = gateways::active()?;
    let pat_store = crate::config::read_file(&profile.config_file)?
        .pat
        .and_then(|p| p.store);
    Ok(PathLayout {
        config_dir: profile.dir.clone(),
        config_file: profile.config_file.clone(),
        pat_file: profile.pat_file(),
        pat_store,
        profile,
    })
}

#[tracing::instrument(level = "debug", skip(token), fields(has_gateway = gateway_url.is_some()))]
pub fn login(token: &str, gateway_url: Option<&str>) -> Result<PathLayout, SetupError> {
    validate_token(token)?;
    let mut paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    let previous = configured_gateway()?;
    let store = credential::select_store();
    credential::write(&paths.profile, store, token)?;
    if let Some(old) = paths.pat_store.filter(|old| *old != store) {
        credential::delete(&paths.profile, old)?;
    }
    write_config_file(&paths.config_file, store, gateway_url)?;
    paths.pat_store = Some(store);
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
    let gateway = resolve_gateway(&paths.config_file, Some(trimmed))?;
    let store = paths.pat_store;
    merge_config_file(&paths.config_file, &gateway, "pat", |doc| {
        store.map_or(Ok(()), |store| {
            write::set(doc, &["pat", "store"], store.as_str())
        })
    })?;
    invalidate_cached_token()?;
    forget_gateway_state_if_moved(&previous)?;
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn logout(secrets: &crate::auth::plugin_oauth::SecretStore) -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    if let Some(store) = paths.pat_store {
        credential::delete(&paths.profile, store)?;
    }
    remove_managed_mcp_fragment()?;
    remove_sync_state()?;
    crate::auth::cache::clear().map_err(SetupError::Cache)?;
    crate::auth::plugin_oauth::delete_creds(secrets)?;
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
pub fn clean(secrets: &crate::auth::plugin_oauth::SecretStore) -> Result<CleanReport, SetupError> {
    let paths = resolve_paths()?;
    let pat_removed = paths.pat_store.is_some();
    if let Some(store) = paths.pat_store {
        credential::delete(&paths.profile, store)?;
    }
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
    crate::auth::plugin_oauth::delete_creds(secrets)?;
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
    let pat_present = paths.pat_store.is_some();
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
        write::set(doc, &["session", "enabled"], true)?;
        write::set(
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
