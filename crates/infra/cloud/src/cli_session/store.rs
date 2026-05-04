//! On-disk index of CLI sessions keyed by tenant or `local`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;

use super::{CliSession, LOCAL_SESSION_KEY, SessionKey};
use crate::error::CloudResult;

const STORE_VERSION: u32 = 1;

/// Persistent map of session keys to [`CliSession`] records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStore {
    /// On-disk schema version.
    pub version: u32,
    /// Map of storage key to session record.
    pub sessions: HashMap<String, CliSession>,
    /// Currently active session, if any.
    pub active_key: Option<String>,
    /// Active profile name (mirrored from the session for the CLI
    /// status line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile_name: Option<String>,
    /// Last write timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    /// Create an empty session store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: STORE_VERSION,
            sessions: HashMap::new(),
            active_key: None,
            active_profile_name: None,
            updated_at: Utc::now(),
        }
    }

    /// Borrow a session if it exists, is unexpired, and has valid
    /// credentials.
    #[must_use]
    pub fn get_valid_session(&self, key: &SessionKey) -> Option<&CliSession> {
        self.sessions
            .get(&key.as_storage_key())
            .filter(|s| !s.is_expired() && s.has_valid_credentials())
    }

    /// Mutable variant of [`SessionStore::get_valid_session`].
    pub fn get_valid_session_mut(&mut self, key: &SessionKey) -> Option<&mut CliSession> {
        self.sessions
            .get_mut(&key.as_storage_key())
            .filter(|s| !s.is_expired() && s.has_valid_credentials())
    }

    /// Borrow a session regardless of expiry/credentials.
    #[must_use]
    pub fn get_session(&self, key: &SessionKey) -> Option<&CliSession> {
        self.sessions.get(&key.as_storage_key())
    }

    /// Insert or replace a session.
    pub fn upsert_session(&mut self, key: &SessionKey, session: CliSession) {
        self.sessions.insert(key.as_storage_key(), session);
        self.updated_at = Utc::now();
    }

    /// Remove a session and return the previous value, if any.
    pub fn remove_session(&mut self, key: &SessionKey) -> Option<CliSession> {
        let storage_key = key.as_storage_key();
        let removed = self.sessions.remove(&storage_key);
        if removed.is_some() {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Mark `key` as the active session.
    pub fn set_active(&mut self, key: &SessionKey) {
        self.active_key = Some(key.as_storage_key());
        self.updated_at = Utc::now();
    }

    /// Mark `key` as active and record the profile name.
    pub fn set_active_with_profile(&mut self, key: &SessionKey, profile_name: &str) {
        self.active_key = Some(key.as_storage_key());
        self.active_profile_name = Some(profile_name.to_string());
        self.updated_at = Utc::now();
    }

    /// Mark `key` as active, set the profile name, and update the
    /// session's profile path.
    pub fn set_active_with_profile_path(
        &mut self,
        key: &SessionKey,
        profile_name: &str,
        profile_path: PathBuf,
    ) {
        self.active_key = Some(key.as_storage_key());
        self.active_profile_name = Some(profile_name.to_string());

        if let Some(session) = self.sessions.get_mut(&key.as_storage_key()) {
            session.update_profile_path(profile_path);
        }

        self.updated_at = Utc::now();
    }

    /// Decode `active_key` back into a typed [`SessionKey`].
    #[must_use]
    pub fn active_session_key(&self) -> Option<SessionKey> {
        self.active_key.as_ref().map(|k| {
            if k == LOCAL_SESSION_KEY {
                SessionKey::Local
            } else {
                k.strip_prefix("tenant_").map_or(SessionKey::Local, |id| {
                    SessionKey::Tenant(TenantId::new(id))
                })
            }
        })
    }

    /// Borrow the currently active session if it is valid.
    #[must_use]
    pub fn active_session(&self) -> Option<&CliSession> {
        self.active_session_key()
            .and_then(|key| self.get_valid_session(&key))
    }

    /// Drop expired sessions and return the number removed.
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
        }

        if count > 0 {
            self.updated_at = Utc::now();
        }
        count
    }

    /// Find the first unexpired session matching `name`.
    #[must_use]
    pub fn find_by_profile_name(&self, name: &str) -> Option<&CliSession> {
        self.sessions
            .values()
            .find(|s| s.profile_name.as_str() == name && !s.is_expired())
    }

    /// Iterate all sessions as `(storage_key, session)` pairs.
    #[must_use]
    pub fn all_sessions(&self) -> Vec<(&String, &CliSession)> {
        self.sessions.iter().collect()
    }

    /// Number of sessions in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// `true` if the store has no sessions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Read `index.json` from `sessions_dir`, returning `None` when
    /// the file is missing or corrupt. Failures are logged at
    /// `debug` / `warn`.
    #[must_use]
    pub fn load(sessions_dir: &Path) -> Option<Self> {
        let index_path = sessions_dir.join("index.json");
        let content = match fs::read_to_string(&index_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(error = %e, "No session store found");
                return None;
            },
        };
        match serde_json::from_str(&content) {
            Ok(store) => Some(store),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse session store");
                None
            },
        }
    }

    /// Read the store from disk, returning a fresh empty store when
    /// the file is missing or corrupt.
    ///
    /// # Errors
    ///
    /// Currently infallible; the [`CloudResult`] return is kept for
    /// forward compatibility — future versions may surface
    /// validation failures here.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Preserves the existing public signature for callers using `?`"
    )]
    pub fn load_or_create(sessions_dir: &Path) -> CloudResult<Self> {
        Ok(Self::load(sessions_dir).unwrap_or_default())
    }

    /// Atomically write the store back to `index.json` with `0o600`
    /// permissions on Unix.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CloudError::Io`] /
    /// [`Json`](crate::error::CloudError::Json) for filesystem and
    /// serialization failures.
    pub fn save(&self, sessions_dir: &Path) -> CloudResult<()> {
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
