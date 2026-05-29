//! The [`CliSession`] record, its [`CliSessionBuilder`], and on-disk load/save.
//!
//! A session bundles the authenticated [`SessionIdentity`] with its token,
//! profile binding, and expiry. Persistence writes a `0600` file under a
//! `.gitignore`-protected directory and rejects on-disk versions outside the
//! supported range.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId,
};
use systemprompt_models::auth::UserType;

use super::{LOCAL_SESSION_KEY, SessionKey};
use crate::error::{CloudError, CloudResult};

const CURRENT_VERSION: u32 = 5;
const MIN_SUPPORTED_VERSION: u32 = 5;
const SESSION_DURATION_HOURS: i64 = 24;

/// The authenticated identity a CLI session represents. Bundled so every
/// builder call carries the full triple — there is no longer a default
/// that silently elevates an unbound session to admin.
#[derive(Debug, Clone)]
pub struct SessionIdentity {
    pub user_id: UserId,
    pub email: Email,
    pub user_type: UserType,
}

impl SessionIdentity {
    #[must_use]
    pub const fn new(user_id: UserId, email: Email, user_type: UserType) -> Self {
        Self {
            user_id,
            email,
            user_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliSession {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_key: Option<TenantId>,
    pub profile_name: ProfileName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<PathBuf>,
    pub session_token: SessionToken,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub user_id: UserId,
    pub user_email: Email,
    pub user_type: UserType,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
}

#[derive(Debug)]
pub struct CliSessionBuilder {
    tenant_key: Option<TenantId>,
    profile_name: ProfileName,
    profile_path: Option<PathBuf>,
    session_token: SessionToken,
    session_id: SessionId,
    context_id: ContextId,
    user_id: UserId,
    user_email: Email,
    user_type: UserType,
}

impl CliSessionBuilder {
    pub fn new(
        profile_name: ProfileName,
        session_token: SessionToken,
        session_id: SessionId,
        context_id: ContextId,
        identity: SessionIdentity,
    ) -> Self {
        Self {
            tenant_key: None,
            profile_name,
            profile_path: None,
            session_token,
            session_id,
            context_id,
            user_id: identity.user_id,
            user_email: identity.email,
            user_type: identity.user_type,
        }
    }

    #[must_use]
    pub fn with_tenant_key(mut self, tenant_key: TenantId) -> Self {
        self.tenant_key = Some(tenant_key);
        self
    }

    #[must_use]
    pub fn with_session_key(mut self, key: &SessionKey) -> Self {
        self.tenant_key = match key {
            SessionKey::Local => Some(TenantId::new(LOCAL_SESSION_KEY)),
            SessionKey::Tenant(id) => Some(id.clone()),
        };
        self
    }

    #[must_use]
    pub fn with_profile_path(mut self, profile_path: impl Into<PathBuf>) -> Self {
        self.profile_path = Some(profile_path.into());
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

impl CliSession {
    pub fn builder(
        profile_name: ProfileName,
        session_token: SessionToken,
        session_id: SessionId,
        context_id: ContextId,
        identity: SessionIdentity,
    ) -> CliSessionBuilder {
        CliSessionBuilder::new(
            profile_name,
            session_token,
            session_id,
            context_id,
            identity,
        )
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.context_id
    }

    pub fn touch(&mut self) {
        self.last_used = Utc::now();
    }

    pub fn set_context_id(&mut self, context_id: ContextId) {
        self.context_id = context_id;
        self.last_used = Utc::now();
    }

    pub fn update_profile_path(&mut self, profile_path: PathBuf) {
        self.profile_path = Some(profile_path);
        self.last_used = Utc::now();
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    #[must_use]
    pub fn is_valid_for_profile(&self, profile_name: &str) -> bool {
        self.profile_name.as_str() == profile_name && !self.is_expired()
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
            (SessionKey::Local, Some(k)) => k.as_str() == LOCAL_SESSION_KEY,
            (SessionKey::Tenant(id), Some(k)) => k == id,
            (SessionKey::Tenant(_), None) => false,
        }
    }

    #[must_use]
    pub fn session_key(&self) -> SessionKey {
        match &self.tenant_key {
            None => SessionKey::Local,
            Some(k) if k.as_str() == LOCAL_SESSION_KEY => SessionKey::Local,
            Some(k) => SessionKey::Tenant(k.clone()),
        }
    }

    pub fn load_from_path(path: &Path) -> CloudResult<Self> {
        if !path.exists() {
            return Err(CloudError::NotAuthenticated);
        }

        let content = fs::read_to_string(path)?;

        let mut session: Self = serde_json::from_str(&content)
            .map_err(|e| CloudError::CredentialsCorrupted { source: e })?;

        if session.version < MIN_SUPPORTED_VERSION || session.version > CURRENT_VERSION {
            return Err(CloudError::SessionVersionMismatch {
                min: MIN_SUPPORTED_VERSION,
                max: CURRENT_VERSION,
                actual: session.version,
                path: path.display().to_string(),
            });
        }

        session.version = CURRENT_VERSION;
        Ok(session)
    }

    pub fn save_to_path(&self, path: &Path) -> CloudResult<()> {
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

    pub fn delete_from_path(path: &Path) -> CloudResult<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
