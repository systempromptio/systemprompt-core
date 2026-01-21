use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use systemprompt_identifiers::TenantId;

use super::{CliSession, SessionKey, LOCAL_SESSION_KEY};

const STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStore {
    pub version: u32,
    pub sessions: HashMap<String, CliSession>,
    pub active_key: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: STORE_VERSION,
            sessions: HashMap::new(),
            active_key: None,
            updated_at: Utc::now(),
        }
    }

    #[must_use]
    pub fn get_valid_session(&self, key: &SessionKey) -> Option<&CliSession> {
        self.sessions
            .get(&key.as_storage_key())
            .filter(|s| !s.is_expired() && s.has_valid_credentials())
    }

    pub fn get_valid_session_mut(&mut self, key: &SessionKey) -> Option<&mut CliSession> {
        self.sessions
            .get_mut(&key.as_storage_key())
            .filter(|s| !s.is_expired() && s.has_valid_credentials())
    }

    #[must_use]
    pub fn get_session(&self, key: &SessionKey) -> Option<&CliSession> {
        self.sessions.get(&key.as_storage_key())
    }

    pub fn upsert_session(&mut self, key: &SessionKey, session: CliSession) {
        self.sessions.insert(key.as_storage_key(), session);
        self.updated_at = Utc::now();
    }

    pub fn remove_session(&mut self, key: &SessionKey) -> Option<CliSession> {
        let storage_key = key.as_storage_key();
        let removed = self.sessions.remove(&storage_key);
        if removed.is_some() {
            self.updated_at = Utc::now();
            if self.active_key.as_ref() == Some(&storage_key) {
                self.active_key = None;
            }
        }
        removed
    }

    pub fn set_active(&mut self, key: &SessionKey) {
        self.active_key = Some(key.as_storage_key());
        self.updated_at = Utc::now();
    }

    #[must_use]
    pub fn active_session_key(&self) -> Option<SessionKey> {
        self.active_key.as_ref().map(|k| {
            if k == LOCAL_SESSION_KEY {
                SessionKey::Local
            } else {
                k.strip_prefix("tenant_")
                    .map(|id| SessionKey::Tenant(TenantId::new(id)))
                    .unwrap_or(SessionKey::Local)
            }
        })
    }

    #[must_use]
    pub fn active_session(&self) -> Option<&CliSession> {
        self.active_session_key()
            .and_then(|key| self.get_valid_session(&key))
    }

    pub fn prune_expired(&mut self) -> usize {
        let expired_keys: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired_keys.len();
        for key in &expired_keys {
            self.sessions.remove(key);
            if self.active_key.as_ref() == Some(key) {
                self.active_key = None;
            }
        }

        if count > 0 {
            self.updated_at = Utc::now();
        }
        count
    }

    #[must_use]
    pub fn all_sessions(&self) -> Vec<(&String, &CliSession)> {
        self.sessions.iter().collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn load_or_create(sessions_dir: &Path, legacy_session_path: Option<&Path>) -> Result<Self> {
        let index_path = sessions_dir.join("index.json");

        if index_path.exists() {
            let content = fs::read_to_string(&index_path)
                .with_context(|| format!("Failed to read {}", index_path.display()))?;
            return serde_json::from_str(&content)
                .with_context(|| "Failed to parse session store index");
        }

        let mut store = Self::new();

        if let Some(legacy_path) = legacy_session_path.filter(|p| p.exists()) {
            if let Ok(legacy_session) = CliSession::load_from_path(legacy_path) {
                let key = legacy_session.session_key();
                store.upsert_session(&key, legacy_session);
                store.set_active(&key);
                store.save(sessions_dir)?;
                if let Err(e) = fs::remove_file(legacy_path) {
                    tracing::warn!(error = %e, path = %legacy_path.display(), "Failed to remove legacy session file");
                }
            }
        }

        Ok(store)
    }

    pub fn save(&self, sessions_dir: &Path) -> Result<()> {
        fs::create_dir_all(sessions_dir)?;

        let gitignore_path = sessions_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, "*\n")?;
        }

        let index_path = sessions_dir.join("index.json");
        let content = serde_json::to_string_pretty(self)?;
        let temp_path = index_path.with_extension("tmp");
        fs::write(&temp_path, &content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&temp_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&temp_path, perms)?;
        }

        fs::rename(&temp_path, &index_path)?;
        Ok(())
    }
}
