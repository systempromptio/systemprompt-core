use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use systemprompt_models::profile::{CloudConfig, CloudValidationMode};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use crate::CloudCredentials;

static CREDENTIALS: OnceLock<Option<CloudCredentials>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct CredentialsBootstrap;

#[derive(Debug, thiserror::Error)]
pub enum CredentialsBootstrapError {
    #[error(
        "Credentials not initialized. Call CredentialsBootstrap::init() after \
         ProfileBootstrap::init()"
    )]
    NotInitialized,

    #[error("Credentials already initialized")]
    AlreadyInitialized,

    #[error("Profile not initialized. Call ProfileBootstrap::init() first")]
    ProfileNotInitialized,

    #[error("Cloud credentials not available (cloud disabled or not configured)")]
    NotAvailable,

    #[error("Cloud credentials file not found: {path}")]
    FileNotFound { path: String },

    #[error("Cloud credentials file invalid: {message}")]
    InvalidCredentials { message: String },

    #[error("Cloud token has expired. Run 'systemprompt cloud login' to refresh")]
    TokenExpired,
}

impl CredentialsBootstrap {
    pub fn init() -> Result<Option<&'static CloudCredentials>> {
        if CREDENTIALS.get().is_some() {
            anyhow::bail!(CredentialsBootstrapError::AlreadyInitialized);
        }

        if Self::is_fly_container() {
            tracing::debug!("Fly.io container detected, loading credentials from environment");
            let creds = Self::load_from_env();
            CREDENTIALS
                .set(creds)
                .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
            return Ok(CREDENTIALS
                .get()
                .ok_or(CredentialsBootstrapError::NotInitialized)?
                .as_ref());
        }

        let profile = ProfileBootstrap::get()
            .map_err(|_| CredentialsBootstrapError::ProfileNotInitialized)?;

        let Some(cloud_config) = &profile.cloud else {
            CREDENTIALS
                .set(None)
                .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
            return Ok(None);
        };

        let credentials_path = Self::resolve_credentials_path(cloud_config)?;

        match Self::load_credentials(&credentials_path, cloud_config.validation) {
            Ok(creds) => {
                CREDENTIALS
                    .set(Some(creds))
                    .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
                Ok(CREDENTIALS
                    .get()
                    .ok_or(CredentialsBootstrapError::NotInitialized)?
                    .as_ref())
            },
            Err(e) => match cloud_config.validation {
                CloudValidationMode::Strict => Err(e),
                CloudValidationMode::Warn => {
                    tracing::warn!("Cloud credentials issue (continuing anyway): {}", e);
                    CREDENTIALS
                        .set(None)
                        .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
                    Ok(None)
                },
                CloudValidationMode::Skip => {
                    tracing::debug!("Skipping cloud credentials validation: {}", e);
                    CREDENTIALS
                        .set(None)
                        .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
                    Ok(None)
                },
            },
        }
    }

    fn is_fly_container() -> bool {
        std::env::var("FLY_APP_NAME").is_ok()
    }

    fn load_from_env() -> Option<CloudCredentials> {
        let api_token = std::env::var("SYSTEMPROMPT_API_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())?;

        tracing::debug!("Loading cloud credentials from environment variables");

        Some(CloudCredentials {
            api_token,
            api_url: std::env::var("SYSTEMPROMPT_API_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "https://api.systemprompt.io".into()),
            authenticated_at: Utc::now(),
            user_email: std::env::var("SYSTEMPROMPT_USER_EMAIL")
                .ok()
                .filter(|s| !s.is_empty()),
        })
    }

    pub fn get() -> Result<Option<&'static CloudCredentials>, CredentialsBootstrapError> {
        CREDENTIALS
            .get()
            .map(|opt| opt.as_ref())
            .ok_or(CredentialsBootstrapError::NotInitialized)
    }

    pub fn require() -> Result<&'static CloudCredentials, CredentialsBootstrapError> {
        Self::get()?.ok_or(CredentialsBootstrapError::NotAvailable)
    }

    pub fn is_initialized() -> bool {
        CREDENTIALS.get().is_some()
    }

    pub fn try_init() -> Result<Option<&'static CloudCredentials>> {
        if CREDENTIALS.get().is_some() {
            return Self::get().map_err(Into::into);
        }
        Self::init()
    }

    pub fn expires_within(duration: chrono::Duration) -> bool {
        Self::get()
            .ok()
            .flatten()
            .is_some_and(|c| c.expires_within(duration))
    }

    pub fn reload() -> Result<CloudCredentials, CredentialsBootstrapError> {
        let profile = ProfileBootstrap::get()
            .map_err(|_| CredentialsBootstrapError::ProfileNotInitialized)?;

        let cloud_config = profile
            .cloud
            .as_ref()
            .ok_or(CredentialsBootstrapError::NotAvailable)?;

        let path = Self::resolve_credentials_path(cloud_config).map_err(|e| {
            CredentialsBootstrapError::InvalidCredentials {
                message: e.to_string(),
            }
        })?;

        Self::load_credentials(&path, cloud_config.validation).map_err(|e| {
            CredentialsBootstrapError::InvalidCredentials {
                message: e.to_string(),
            }
        })
    }

    fn resolve_credentials_path(cloud_config: &CloudConfig) -> Result<PathBuf> {
        let profile_path = ProfileBootstrap::get_path()
            .map_err(|_| CredentialsBootstrapError::ProfileNotInitialized)?;
        let profile_dir = Path::new(profile_path)
            .parent()
            .context("Invalid profile path")?;

        Ok(crate::paths::resolve_path(
            profile_dir,
            &cloud_config.credentials_path,
        ))
    }

    fn load_credentials(path: &Path, validation: CloudValidationMode) -> Result<CloudCredentials> {
        let creds = CloudCredentials::load_from_path(path).map_err(|e| {
            if path.exists() {
                anyhow::anyhow!(CredentialsBootstrapError::InvalidCredentials {
                    message: e.to_string(),
                })
            } else {
                anyhow::anyhow!(CredentialsBootstrapError::FileNotFound {
                    path: path.display().to_string()
                })
            }
        })?;

        if validation != CloudValidationMode::Skip && creds.is_token_expired() {
            anyhow::bail!(CredentialsBootstrapError::TokenExpired);
        }

        if creds.expires_within(chrono::Duration::hours(1)) {
            tracing::warn!(
                "Cloud token will expire soon. Consider running 'systemprompt cloud login' to \
                 refresh."
            );
        }

        tracing::debug!(
            "Loaded cloud credentials from {} (user: {:?})",
            path.display(),
            creds.user_email
        );

        Ok(creds)
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub mod test_helpers {
    use chrono::{Duration, Utc};

    use crate::CloudCredentials;

    pub fn valid_credentials() -> CloudCredentials {
        CloudCredentials {
            api_token: "test_token_valid_eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".into(),
            api_url: "https://api.test.systemprompt.io".into(),
            authenticated_at: Utc::now(),
            user_email: Some("test@example.com".into()),
        }
    }

    pub fn expired_credentials() -> CloudCredentials {
        let mut creds = valid_credentials();
        creds.authenticated_at = Utc::now() - Duration::days(30);
        creds
    }

    pub fn expiring_soon_credentials() -> CloudCredentials {
        let mut creds = valid_credentials();
        creds.authenticated_at = Utc::now() - Duration::hours(23);
        creds
    }

    pub fn minimal_credentials() -> CloudCredentials {
        CloudCredentials {
            api_token: "minimal_test_token".into(),
            api_url: "https://api.systemprompt.io".into(),
            authenticated_at: Utc::now(),
            user_email: None,
        }
    }
}
