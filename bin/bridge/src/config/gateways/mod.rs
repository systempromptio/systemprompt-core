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

mod cached;
pub mod credential;
mod error;
mod id;
mod index;
pub mod migrate;

use std::path::{Path, PathBuf};

use serde::Serialize;
use systemprompt_identifiers::ValidatedUrl;

pub use cached::{
    CachedDiscovery, DeliveredInstance, ServedModels, delivered_windows, forget_discovery,
    model_union, models_served_elsewhere, profile_trust_pins, read_delivered, read_discovery,
    read_served_models, served_models_by_profile, write_delivered, write_discovery,
    write_served_models,
};
pub use error::ProfileError;
pub use id::{GatewayProfileId, ProfileIdError};

pub const GATEWAYS_DIR: &str = "gateways";
pub const DISCOVERY_FILE: &str = "discovery.json";
pub const WINDOW_FILE: &str = "release-window.json";
pub const MODELS_FILE: &str = "models.json";

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
