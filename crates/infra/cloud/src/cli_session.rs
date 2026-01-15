use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::{ContextId, SessionId};

use crate::error::CloudError;

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSession {
    pub version: u32,
    pub context_id: ContextId,
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
}

impl CliSession {
    #[must_use]
    pub fn new(context_id: ContextId, session_id: SessionId) -> Self {
        let now = Utc::now();
        Self {
            version: CURRENT_VERSION,
            context_id,
            session_id,
            created_at: now,
            last_used: now,
        }
    }

    pub fn touch(&mut self) {
        self.last_used = Utc::now();
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
                "Session file version mismatch: expected {}, got {}",
                CURRENT_VERSION,
                session.version
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
