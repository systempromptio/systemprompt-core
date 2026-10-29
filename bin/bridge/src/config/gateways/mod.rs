//! Per-gateway profiles: one directory per gateway this install can sign in
//! to, and an index naming the active one.
//!
//! ```text
//! <config_dir>/<brand.config_dir>/
//!   <brand.config_file>          index: `active = "<id>"`
//!   gateways/<id>/
//!     <brand.config_file>        gateway URL, credential section, `[sync.trust]`,
//!                                `[instance] disabled`
//!     <brand.pat_file>           the PAT when `pat.store = "file"`
//!     discovery.json             the gateway's `/.well-known/systemprompt-bridge`
//!     release-window.json        the release window its last verified manifest set
//! ```
//!
//! Synced host state (the metadata directory) is machine-wide and always
//! belongs to the active profile: every fragment there is stamped with its
//! gateway, and a switch clears it before the next profile syncs. An explicit
//! `<ENV_PREFIX>_CONFIG` pins the bridge to that one file; profile commands
//! then refuse rather than write a layout the pinned process never reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod credential;
mod id;
mod index;
pub mod migrate;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, InstanceBranding};

pub use id::{GatewayProfileId, ProfileIdError};

pub const GATEWAYS_DIR: &str = "gateways";
pub const DISCOVERY_FILE: &str = "discovery.json";
pub const WINDOW_FILE: &str = "release-window.json";
pub const MODELS_FILE: &str = "models.json";

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("config path unresolvable on this platform")]
    PathUnresolvable,
    #[error(
        "this process is pinned to {path} by {var}; gateway profiles are unavailable while it is set"
    )]
    Pinned { path: PathBuf, var: String },
    #[error(transparent)]
    Id(#[from] ProfileIdError),
    #[error("no gateway profile named `{0}`")]
    Unknown(GatewayProfileId),
    #[error("`{0}` is the active gateway; switch to another before removing it")]
    RemoveActive(GatewayProfileId),
    #[error("a gateway profile for {0} already exists as `{1}`")]
    Duplicate(String, GatewayProfileId),
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    Malformed {
        path: PathBuf,
        #[source]
        source: Box<toml_edit::TomlError>,
    },
    #[error("{path}: `active` must name a gateway profile: {source}")]
    BadActive {
        path: PathBuf,
        #[source]
        source: ProfileIdError,
    },
    #[error("{path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    ConfigWrite(#[from] super::ConfigWriteError),
    #[error(transparent)]
    ConfigRead(#[from] super::ConfigReadError),
    #[error(transparent)]
    Credential(#[from] credential::CredentialError),
}

impl ProfileError {
    fn io(action: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
}

/// The profile every per-gateway path resolves through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveProfile {
    pub id: GatewayProfileId,
    pub dir: PathBuf,
    pub config_file: PathBuf,
    pub pinned: bool,
}

impl ActiveProfile {
    #[must_use]
    pub fn pat_file(&self) -> PathBuf {
        self.dir.join(crate::brand::brand().pat_file)
    }

    #[must_use]
    pub fn discovery_file(&self) -> PathBuf {
        self.dir.join(DISCOVERY_FILE)
    }

    #[must_use]
    pub fn window_file(&self) -> PathBuf {
        self.dir.join(WINDOW_FILE)
    }
}

#[must_use]
pub fn config_root() -> Option<PathBuf> {
    crate::basedirs::config_dir().map(|base| base.join(crate::brand::brand().config_dir))
}

#[must_use]
pub fn index_path() -> Option<PathBuf> {
    config_root().map(|root| root.join(crate::brand::brand().config_file))
}

fn pinned_config() -> Option<PathBuf> {
    std::env::var_os(crate::brand::brand().env("CONFIG")).map(PathBuf::from)
}

fn refuse_if_pinned() -> Result<(), ProfileError> {
    pinned_config().map_or(Ok(()), |path| {
        Err(ProfileError::Pinned {
            path,
            var: crate::brand::brand().env("CONFIG"),
        })
    })
}

#[must_use]
pub fn profile_dir(id: &GatewayProfileId) -> Option<PathBuf> {
    config_root().map(|root| root.join(GATEWAYS_DIR).join(id.as_str()))
}

fn profile_for(id: GatewayProfileId) -> Result<ActiveProfile, ProfileError> {
    let dir = profile_dir(&id).ok_or(ProfileError::PathUnresolvable)?;
    let config_file = dir.join(crate::brand::brand().config_file);
    Ok(ActiveProfile {
        id,
        dir,
        config_file,
        pinned: false,
    })
}

pub fn profile(id: &GatewayProfileId) -> Result<ActiveProfile, ProfileError> {
    let profile = profile_for(id.clone())?;
    if profile.dir.exists() {
        Ok(profile)
    } else {
        Err(ProfileError::Unknown(id.clone()))
    }
}

pub fn active() -> Result<ActiveProfile, ProfileError> {
    if let Some(config_file) = pinned_config() {
        let dir = config_file
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        return Ok(ActiveProfile {
            id: GatewayProfileId::default_profile(),
            dir,
            config_file,
            pinned: true,
        });
    }
    let index = index_path().ok_or(ProfileError::PathUnresolvable)?;
    let id = index::read_active(&index)?.unwrap_or_else(GatewayProfileId::default_profile);
    profile_for(id)
}

/// One row of `gateway list`.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummary {
    pub id: GatewayProfileId,
    pub gateway_url: Option<String>,
    pub active: bool,
    pub disabled: bool,
    pub display_name: Option<String>,
}

pub fn list() -> Result<Vec<ProfileSummary>, ProfileError> {
    let active = active()?;
    if active.pinned {
        return Ok(vec![summary(&active, true)?]);
    }
    let root = config_root()
        .ok_or(ProfileError::PathUnresolvable)?
        .join(GATEWAYS_DIR);
    let mut ids = Vec::new();
    match std::fs::read_dir(&root) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|e| ProfileError::io("list", &root, e))?;
                if !entry.path().is_dir() {
                    continue;
                }
                if let Ok(id) = GatewayProfileId::try_new(entry.file_name().to_string_lossy()) {
                    ids.push(id);
                }
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(ProfileError::io("list", &root, e)),
    }
    if !ids.contains(&active.id) {
        ids.push(active.id.clone());
    }
    ids.sort();
    ids.into_iter()
        .map(|id| {
            let is_active = id == active.id;
            summary(&profile_for(id)?, is_active)
        })
        .collect()
}

fn summary(profile: &ActiveProfile, is_active: bool) -> Result<ProfileSummary, ProfileError> {
    let cfg = super::read_file(&profile.config_file)?;
    let display_name = read_discovery(profile)
        .ok()
        .flatten()
        .and_then(|cached| cached.document.branding.display_name);
    Ok(ProfileSummary {
        id: profile.id.clone(),
        disabled: cfg.is_disabled(),
        gateway_url: cfg.gateway_url.map(|u| u.as_str().to_owned()),
        active: is_active,
        display_name,
    })
}

pub fn add(gateway_url: &str) -> Result<GatewayProfileId, ProfileError> {
    refuse_if_pinned()?;
    let url = ValidatedUrl::try_new(gateway_url.trim())
        .map_err(|e| ProfileError::Id(ProfileIdError::Url(e)))?;
    for existing in list()? {
        if existing
            .gateway_url
            .as_deref()
            .and_then(|u| ValidatedUrl::try_new(u).ok())
            .is_some_and(|u| crate::config::same_origin(&u, &url))
        {
            return Err(ProfileError::Duplicate(
                url.as_str().to_owned(),
                existing.id,
            ));
        }
    }
    let base = GatewayProfileId::from_url(&url);
    let mut candidate = base.clone();
    let mut n: u32 = 2;
    while profile_dir(&candidate).is_some_and(|d| d.exists()) {
        candidate = base.with_suffix(n);
        n += 1;
    }
    let profile = profile_for(candidate.clone())?;
    crate::fsutil::create_dir_all_mode_0700(&profile.dir)
        .map_err(|e| ProfileError::io("create", &profile.dir, e))?;
    super::write::edit_file(&profile.config_file, |doc| {
        super::write::set(doc, &["gateway_url"], url.as_str())
    })?;
    Ok(candidate)
}

pub fn set_active(id: &GatewayProfileId) -> Result<ActiveProfile, ProfileError> {
    refuse_if_pinned()?;
    let profile = profile_for(id.clone())?;
    if !profile.config_file.exists() {
        return Err(ProfileError::Unknown(id.clone()));
    }
    let index = index_path().ok_or(ProfileError::PathUnresolvable)?;
    index::write_active(&index, id)?;
    Ok(profile)
}

pub fn remove(id: &GatewayProfileId) -> Result<(), ProfileError> {
    refuse_if_pinned()?;
    if active()?.id == *id {
        return Err(ProfileError::RemoveActive(id.clone()));
    }
    let profile = profile_for(id.clone())?;
    if !profile.dir.exists() {
        return Err(ProfileError::Unknown(id.clone()));
    }
    let cfg = super::read_file(&profile.config_file)?;
    if let Some(store) = cfg.pat.as_ref().and_then(|p| p.store) {
        credential::delete(&profile, store)?;
    }
    std::fs::remove_dir_all(&profile.dir).map_err(|e| ProfileError::io("remove", &profile.dir, e))
}

/// The discovery document cached for a profile, stamped with the gateway that
/// served it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDiscovery {
    pub gateway: ValidatedUrl,
    pub document: systemprompt_models::bridge::instance::BridgeDiscovery,
}

pub fn read_discovery(profile: &ActiveProfile) -> Result<Option<CachedDiscovery>, ProfileError> {
    read_json(&profile.discovery_file())
}

pub fn write_discovery(
    profile: &ActiveProfile,
    cached: &CachedDiscovery,
) -> Result<(), ProfileError> {
    write_json(&profile.discovery_file(), cached)
}

pub fn forget_discovery(profile: &ActiveProfile) -> Result<(), ProfileError> {
    let path = profile.discovery_file();
    crate::fsutil::remove_verified(&path).map_err(|e| ProfileError::io("remove", &path, e))
}

/// The release window and presentation a profile's last verified manifest
/// delivered.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeliveredInstance {
    pub gateway: Option<ValidatedUrl>,
    #[serde(default)]
    pub window: Option<BridgeVersionWindow>,
    #[serde(default)]
    pub branding: Option<InstanceBranding>,
    #[serde(default)]
    pub allow_user_disable: bool,
}

pub fn read_delivered(profile: &ActiveProfile) -> Result<Option<DeliveredInstance>, ProfileError> {
    read_json(&profile.window_file())
}

pub fn write_delivered(
    profile: &ActiveProfile,
    delivered: &DeliveredInstance,
) -> Result<(), ProfileError> {
    write_json(&profile.window_file(), delivered)
}

pub fn delivered_windows() -> Result<Vec<(GatewayProfileId, BridgeVersionWindow)>, ProfileError> {
    let active = active()?;
    let profiles = if active.pinned {
        vec![active]
    } else {
        list()?
            .into_iter()
            .map(|s| profile_for(s.id))
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut out = Vec::new();
    for profile in profiles {
        if let Some(window) = read_delivered(&profile)?.and_then(|d| d.window) {
            out.push((profile.id, window));
        }
    }
    Ok(out)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, ProfileError> {
    let Some(body) =
        crate::fsutil::read_optional(path).map_err(|e| ProfileError::io("read", path, e))?
    else {
        return Ok(None);
    };
    serde_json::from_str(&body)
        .map(Some)
        .map_err(|source| ProfileError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProfileError> {
    if let Some(parent) = path.parent() {
        crate::fsutil::create_dir_all_mode_0700(parent)
            .map_err(|e| ProfileError::io("create", parent, e))?;
    }
    let body = serde_json::to_vec_pretty(value).map_err(|source| ProfileError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    crate::fsutil::atomic_write_0600(path, &body).map_err(|e| ProfileError::io("write", path, e))
}

pub fn profile_trust_pins() -> Result<Vec<super::TrustRecord>, ProfileError> {
    let mut pins = Vec::new();
    for summary in list()? {
        let profile = profile_for(summary.id)?;
        let cfg = super::read_file(&profile.config_file)?;
        if let Some(record) = cfg.sync.and_then(|s| s.trust) {
            pins.push(record);
        }
    }
    Ok(pins)
}

/// The models a profile's gateway last served, stamped with that gateway.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServedModels {
    pub gateway: Option<ValidatedUrl>,
    #[serde(default)]
    pub models: Vec<String>,
}

pub fn write_served_models(
    profile: &ActiveProfile,
    served: &ServedModels,
) -> Result<(), ProfileError> {
    write_json(&profile.dir.join(MODELS_FILE), served)
}

pub fn read_served_models(profile: &ActiveProfile) -> Result<Option<ServedModels>, ProfileError> {
    read_json(&profile.dir.join(MODELS_FILE))
}

pub fn served_models_by_profile() -> Result<Vec<(GatewayProfileId, Vec<String>)>, ProfileError> {
    let active = active()?;
    if active.pinned {
        return Ok(read_served_models(&active)?
            .map(|s| vec![(active.id.clone(), s.models)])
            .unwrap_or_default());
    }
    let mut out = Vec::new();
    for summary in list()? {
        let profile = profile_for(summary.id)?;
        if let Some(served) = read_served_models(&profile)? {
            out.push((profile.id, served.models));
        }
    }
    Ok(out)
}

#[must_use]
pub fn models_served_elsewhere(
    active: &GatewayProfileId,
    by_profile: &[(GatewayProfileId, Vec<String>)],
) -> std::collections::BTreeMap<String, GatewayProfileId> {
    let active_models: std::collections::BTreeSet<&str> = by_profile
        .iter()
        .filter(|(id, _)| id == active)
        .flat_map(|(_, models)| models.iter().map(String::as_str))
        .collect();
    let mut out = std::collections::BTreeMap::new();
    for (id, models) in by_profile.iter().filter(|(id, _)| id != active) {
        for model in models {
            if !active_models.contains(model.as_str()) {
                out.entry(model.clone()).or_insert_with(|| id.clone());
            }
        }
    }
    out
}

#[must_use]
pub fn model_union(by_profile: &[(GatewayProfileId, Vec<String>)]) -> Vec<String> {
    let mut all: Vec<String> = by_profile
        .iter()
        .flat_map(|(_, models)| models.iter().cloned())
        .collect();
    all.sort();
    all.dedup();
    all
}
