//! Per-profile cached JSON state: the discovery document, the delivered
//! release window and branding, and the models each gateway served.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, InstanceBranding};

use super::{
    ActiveProfile, GatewayProfileId, MODELS_FILE, ProfileError, active, list, profile_for,
};

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

pub fn profile_trust_pins() -> Result<Vec<super::super::TrustRecord>, ProfileError> {
    let mut pins = Vec::new();
    for summary in list()? {
        let profile = profile_for(summary.id)?;
        let cfg = super::super::read_file(&profile.config_file)?;
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
