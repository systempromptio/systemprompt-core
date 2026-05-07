use std::collections::HashMap;
use std::path::PathBuf;
use std::{fs, io};

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

pub fn save_from_manifest(enabled_hosts: &[String]) -> io::Result<()> {
    let mut state = AgentsState::default();
    for host in enabled_hosts {
        state.set_enabled(host, true);
    }
    save(&state)
}

pub(crate) fn save(state: &AgentsState) -> io::Result<()> {
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
