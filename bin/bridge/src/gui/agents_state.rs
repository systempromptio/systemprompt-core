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
            .filter_map(|(k, v)| v.then(|| k.clone()))
            .collect()
    }
}

fn store_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(base.join("systemprompt").join(FILENAME))
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
