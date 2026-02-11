use anyhow::Result;
use chrono::Utc;
use std::path::Path;
use std::sync::OnceLock;

use crate::{CloudApiClient, CloudCredentials};

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

    #[error("Cloud credentials not available")]
    NotAvailable,

    #[error("Cloud credentials file not found: {path}")]
    FileNotFound { path: String },

    #[error("Cloud credentials file invalid: {message}")]
    InvalidCredentials { message: String },

    #[error("Cloud token has expired. Run 'systemprompt cloud login' to refresh")]
    TokenExpired,

    #[error("Cloud API validation failed: {message}")]
    ApiValidationFailed { message: String },
}

impl CredentialsBootstrap {
    pub async fn init() -> Result<Option<&'static CloudCredentials>> {
        if CREDENTIALS.get().is_some() {
            anyhow::bail!(CredentialsBootstrapError::AlreadyInitialized);
        }

        if Self::is_fly_container() {
            tracing::debug!("Fly.io container detected, loading credentials from environment");
            let creds = Self::load_from_env();
            if let Some(ref c) = creds {
                if let Err(e) = Self::validate_with_api(c).await {
                    tracing::warn!(
                        error = %e,
                        "Cloud credential validation failed on Fly.io, continuing with unvalidated credentials"
                    );
                }
            }
            CREDENTIALS
                .set(creds)
                .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
            return Ok(CREDENTIALS
                .get()
                .ok_or(CredentialsBootstrapError::NotInitialized)?
                .as_ref());
        }

        let cloud_paths = crate::paths::get_cloud_paths()?;
        let credentials_path = cloud_paths.resolve(crate::paths::CloudPath::Credentials);

        let creds = Self::load_credentials_from_path(&credentials_path)?;
        Self::validate_with_api(&creds).await?;

        CREDENTIALS
            .set(Some(creds))
            .map_err(|_| CredentialsBootstrapError::AlreadyInitialized)?;
        Ok(CREDENTIALS
            .get()
            .ok_or(CredentialsBootstrapError::NotInitialized)?
            .as_ref())
    }

    async fn validate_with_api(creds: &CloudCredentials) -> Result<()> {
        let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;
        client.get_user().await.map_err(|e| {
            anyhow::anyhow!(CredentialsBootstrapError::ApiValidationFailed {
                message: e.to_string()
            })
        })?;
        tracing::debug!("Cloud credentials validated with API");
        Ok(())
    }

    fn is_fly_container() -> bool {
        std::env::var("FLY_APP_NAME").is_ok()
    }

    fn load_from_env() -> Option<CloudCredentials> {
        let api_token = std::env::var("SYSTEMPROMPT_API_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())?;

        let user_email = std::env::var("SYSTEMPROMPT_USER_EMAIL")
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
            user_email,
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

    pub async fn try_init() -> Result<Option<&'static CloudCredentials>> {
        if CREDENTIALS.get().is_some() {
            return Self::get().map_err(Into::into);
        }
        Self::init().await
    }

    pub fn expires_within(duration: chrono::Duration) -> bool {
        match Self::get() {
            Ok(Some(c)) => c.expires_within(duration),
            Ok(None) => false,
            Err(e) => {
                tracing::debug!(error = %e, "Credentials not available for expiry check");
                false
            },
        }
    }

    pub async fn reload() -> Result<CloudCredentials, CredentialsBootstrapError> {
        let cloud_paths =
            crate::paths::get_cloud_paths().map_err(|_| CredentialsBootstrapError::NotAvailable)?;
        let credentials_path = cloud_paths.resolve(crate::paths::CloudPath::Credentials);

        let creds = Self::load_credentials_from_path(&credentials_path).map_err(|e| {
            CredentialsBootstrapError::InvalidCredentials {
                message: e.to_string(),
            }
        })?;

        Self::validate_with_api(&creds).await.map_err(|e| {
            CredentialsBootstrapError::ApiValidationFailed {
                message: e.to_string(),
            }
        })?;

        Ok(creds)
    }

    fn load_credentials_from_path(path: &Path) -> Result<CloudCredentials> {
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

        if creds.is_token_expired() {
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
            user_email: "test@example.com".into(),
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
            user_email: "minimal@example.com".into(),
        }
    }
}
