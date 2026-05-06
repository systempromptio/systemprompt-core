use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const FILENAME: &str = "agents.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentsState {
    #[serde(default)]
    pub enabled: HashMap<String, bool>,
}

impl AgentsState {
    pub fn is_enabled(&self, host_id: &str) -> bool {
        self.enabled.get(host_id).copied().unwrap_or(false)
    }

    pub fn set_enabled(&mut self, host_id: &str, flag: bool) {
        self.enabled.insert(host_id.to_string(), flag);
    }

    pub fn enabled_ids(&self) -> Vec<String> {
        self.enabled
            .iter()
            .filter(|(_, v)| **v)
            .map(|(k, _)| k.clone())
            .collect()
    }
}

pub fn store_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(base.join("systemprompt").join(FILENAME))
}

pub fn delete() -> io::Result<()> {
    let Some(path) = store_path() else {
        return Ok(());
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn load() -> AgentsState {
    let Some(path) = store_path() else {
        return AgentsState::default();
    };
    let Ok(bytes) = fs::read(&path) else {
        return AgentsState::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn store_exists() -> bool {
    store_path().is_some_and(|p| p.exists())
}

pub fn migrate_from_existing_profiles() -> (AgentsState, Vec<String>) {
    let mut state = AgentsState::default();
    let mut migrated: Vec<String> = Vec::new();
    for host in crate::integration::host_apps() {
        let snap = host.probe();
        let installed = matches!(
            snap.profile_state,
            crate::integration::ProfileState::Installed
        );
        if installed {
            state.set_enabled(host.id(), true);
            migrated.push(host.id().to_string());
        }
    }
    (state, migrated)
}

pub fn save(state: &AgentsState) -> io::Result<()> {
    let path = store_path()
        .ok_or_else(|| io::Error::other("no OS config directory available on this platform"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| io::Error::other(format!("serialise agents state: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}
