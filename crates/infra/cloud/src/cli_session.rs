use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::{ContextId, SessionId, SessionToken, UserId};
use systemprompt_models::auth::UserType;

use crate::error::CloudError;

const CURRENT_VERSION: u32 = 4;
const STORE_VERSION: u32 = 1;
const MIN_SUPPORTED_VERSION: u32 = 3;
const SESSION_DURATION_HOURS: i64 = 24;

pub const LOCAL_SESSION_KEY: &str = "local";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SessionKey {
    Local,
    Tenant(String),
}

impl SessionKey {
    #[must_use]
    pub fn from_tenant_id(tenant_id: Option<&str>) -> Self {
        tenant_id.map_or(Self::Local, |id| Self::Tenant(id.to_string()))
    }

    #[must_use]
    pub fn as_storage_key(&self) -> String {
        match self {
            Self::Local => LOCAL_SESSION_KEY.to_string(),
            Self::Tenant(id) => format!("tenant_{}", id),
        }
    }

    #[must_use]
    pub fn tenant_id(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Tenant(id) => Some(id),
        }
    }

    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

impl std::fmt::Display for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Tenant(id) => write!(f, "tenant:{}", id),
        }
    }
}

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
                    .map(|id| SessionKey::Tenant(id.to_string()))
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
                let key = SessionKey::from_tenant_id(legacy_session.tenant_key.as_deref());
                store.upsert_session(&key, legacy_session);
                store.set_active(&key);
                store.save(sessions_dir)?;
                fs::remove_file(legacy_path).ok();
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSession {
    pub version: u32,
    #[serde(default)]
    pub tenant_key: Option<String>,
    pub profile_name: String,
    #[serde(default)]
    pub profile_path: Option<PathBuf>,
    pub session_token: SessionToken,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub user_id: UserId,
    pub user_email: String,
    #[serde(default = "default_user_type")]
    pub user_type: UserType,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
}

fn default_user_type() -> UserType {
    UserType::Admin
}

#[derive(Debug)]
pub struct CliSessionBuilder {
    tenant_key: Option<String>,
    profile_name: String,
    profile_path: Option<PathBuf>,
    session_token: SessionToken,
    session_id: SessionId,
    context_id: ContextId,
    user_id: UserId,
    user_email: String,
    user_type: UserType,
}

impl CliSessionBuilder {
    pub fn new(
        profile_name: impl Into<String>,
        session_token: SessionToken,
        session_id: SessionId,
        context_id: ContextId,
    ) -> Self {
        Self {
            tenant_key: None,
            profile_name: profile_name.into(),
            profile_path: None,
            session_token,
            session_id,
            context_id,
            user_id: UserId::system(),
            user_email: String::new(),
            user_type: UserType::Admin,
        }
    }

    #[must_use]
    pub fn with_tenant_key(mut self, tenant_key: impl Into<String>) -> Self {
        self.tenant_key = Some(tenant_key.into());
        self
    }

    #[must_use]
    pub fn with_session_key(mut self, key: &SessionKey) -> Self {
        self.tenant_key = Some(match key {
            SessionKey::Local => LOCAL_SESSION_KEY.to_string(),
            SessionKey::Tenant(id) => id.clone(),
        });
        self
    }

    #[must_use]
    pub fn with_profile_path(mut self, profile_path: impl Into<PathBuf>) -> Self {
        self.profile_path = Some(profile_path.into());
        self
    }

    #[must_use]
    pub fn with_user(mut self, user_id: UserId, user_email: impl Into<String>) -> Self {
        self.user_id = user_id;
        self.user_email = user_email.into();
        self
    }

    #[must_use]
    pub const fn with_user_type(mut self, user_type: UserType) -> Self {
        self.user_type = user_type;
        self
    }

    #[must_use]
    pub fn build(self) -> CliSession {
        let now = Utc::now();
        let expires_at = now + Duration::hours(SESSION_DURATION_HOURS);
        CliSession {
            version: CURRENT_VERSION,
            tenant_key: self.tenant_key,
            profile_name: self.profile_name,
            profile_path: self.profile_path,
            session_token: self.session_token,
            session_id: self.session_id,
            context_id: self.context_id,
            user_id: self.user_id,
            user_email: self.user_email,
            user_type: self.user_type,
            created_at: now,
            expires_at,
            last_used: now,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PartialSession {
    #[serde(default)]
    profile_path: Option<PathBuf>,
}

impl CliSession {
    pub fn try_load_profile_path(path: &Path) -> Option<PathBuf> {
        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(path).ok()?;
        let partial: PartialSession = serde_json::from_str(&content).ok()?;
        partial.profile_path.filter(|p| p.exists())
    }

    pub fn builder(
        profile_name: impl Into<String>,
        session_token: SessionToken,
        session_id: SessionId,
        context_id: ContextId,
    ) -> CliSessionBuilder {
        CliSessionBuilder::new(profile_name, session_token, session_id, context_id)
    }

    pub fn context_id(&self) -> &ContextId {
        &self.context_id
    }

    pub fn touch(&mut self) {
        self.last_used = Utc::now();
    }

    pub fn set_context_id(&mut self, context_id: ContextId) {
        self.context_id = context_id;
        self.last_used = Utc::now();
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    #[must_use]
    pub fn is_valid_for_profile(&self, profile_name: &str) -> bool {
        self.profile_name == profile_name && !self.is_expired()
    }

    #[must_use]
    pub fn has_valid_credentials(&self) -> bool {
        !self.session_token.as_str().is_empty()
    }

    #[must_use]
    pub fn is_valid_for_tenant(&self, key: &SessionKey) -> bool {
        if self.is_expired() || !self.has_valid_credentials() {
            return false;
        }

        match (key, &self.tenant_key) {
            (SessionKey::Local, None) => true,
            (SessionKey::Local, Some(k)) => k == LOCAL_SESSION_KEY,
            (SessionKey::Tenant(id), Some(k)) => k == id,
            (SessionKey::Tenant(_), None) => false,
        }
    }

    #[must_use]
    pub fn session_key(&self) -> SessionKey {
        match &self.tenant_key {
            None => SessionKey::Local,
            Some(k) if k == LOCAL_SESSION_KEY => SessionKey::Local,
            Some(k) => SessionKey::Tenant(k.clone()),
        }
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(CloudError::NotAuthenticated.into());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let mut session: Self = serde_json::from_str(&content)
            .map_err(|e| CloudError::CredentialsCorrupted { source: e })?;

        if session.version < MIN_SUPPORTED_VERSION || session.version > CURRENT_VERSION {
            return Err(anyhow::anyhow!(
                "Session file version mismatch: expected {}-{}, got {}. Delete {} and retry.",
                MIN_SUPPORTED_VERSION,
                CURRENT_VERSION,
                session.version,
                path.display()
            ));
        }

        session.version = CURRENT_VERSION;
        Ok(session)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;

            let gitignore_path = dir.join(".gitignore");
            if !gitignore_path.exists() {
                fs::write(&gitignore_path, "*\n")?;
            }
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    pub fn delete_from_path(path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
