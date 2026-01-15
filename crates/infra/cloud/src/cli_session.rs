use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::{ContextId, SessionId, SessionToken, UserId};

use crate::error::CloudError;

const CURRENT_VERSION: u32 = 3;
const SESSION_DURATION_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSession {
    pub version: u32,
    pub profile_name: String,
    pub session_token: SessionToken,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub user_id: UserId,
    pub user_email: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
}

impl CliSession {
    #[must_use]
    pub fn new(
        profile_name: String,
        session_token: SessionToken,
        session_id: SessionId,
        context_id: ContextId,
        user_id: UserId,
        user_email: String,
    ) -> Self {
        let now = Utc::now();
        let expires_at = now + Duration::hours(SESSION_DURATION_HOURS);
        Self {
            version: CURRENT_VERSION,
            profile_name,
            session_token,
            session_id,
            context_id,
            user_id,
            user_email,
            created_at: now,
            expires_at,
            last_used: now,
        }
    }

    pub fn context_id(&self) -> &ContextId {
        &self.context_id
    }

    pub fn touch(&mut self) {
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

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(CloudError::NotAuthenticated.into());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let session: Self = serde_json::from_str(&content)
            .map_err(|e| CloudError::CredentialsCorrupted { source: e })?;

        if session.version != CURRENT_VERSION {
            return Err(anyhow::anyhow!(
                "Session file version mismatch: expected {}, got {}. Delete {} and retry.",
                CURRENT_VERSION,
                session.version,
                path.display()
            ));
        }

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
