//! Process-wide cloud credentials bootstrap.

mod error;

use std::path::Path;
use std::sync::OnceLock;

use chrono::{Duration, Utc};

pub use error::CredentialsBootstrapError;

use crate::error::{CloudError, CloudResult};
use crate::{CloudApiClient, CloudCredentials};

static CREDENTIALS: OnceLock<Option<CloudCredentials>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct CredentialsBootstrap;

impl CredentialsBootstrap {
    pub async fn init() -> CloudResult<Option<&'static CloudCredentials>> {
        if CREDENTIALS.get().is_some() {
            return Err(CredentialsBootstrapError::AlreadyInitialized.into());
        }

        if Self::is_fly_container() {
            tracing::debug!("Fly.io container detected, loading credentials from environment");
            let creds = Self::load_from_env();
            if let Some(ref c) = creds {
                if let Err(e) = Self::validate_with_api(c).await {
                    if Self::allow_unvalidated() {
                        tracing::warn!(
                            target: "security_audit",
                            error = %e,
                            "cloud credentials unvalidated; proceeding under SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1"
                        );
                    } else {
                        return Err(CredentialsBootstrapError::ApiValidationFailed {
                            message: format!(
                                "tenant pod credentials rejected by api.systemprompt.io (token in \
                                 SYSTEMPROMPT_API_TOKEN). Re-run 'systemprompt cloud deploy' or \
                                 set SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1 to bypass. \
                                 Underlying: {e}"
                            ),
                        }
                        .into());
                    }
                }
            }
            CREDENTIALS
                .set(creds)
                .map_err(|_e| CredentialsBootstrapError::AlreadyInitialized)?;
            return Ok(CREDENTIALS
                .get()
                .ok_or(CredentialsBootstrapError::NotInitialized)?
                .as_ref());
        }

        let cloud_paths = crate::paths::get_cloud_paths();
        let credentials_path = cloud_paths.resolve(crate::paths::CloudPath::Credentials);

        let mut creds = Self::load_credentials_from_path(&credentials_path)?;
        if Self::validation_is_fresh(&creds) {
            tracing::debug!("Cloud credentials within validation TTL; skipping API round-trip");
        } else {
            Self::validate_with_api(&creds).await?;
            creds.last_validated_at = Some(Utc::now());
            if let Err(e) = creds.save_to_path(&credentials_path) {
                tracing::debug!(error = %e, "failed to persist credential validation timestamp");
            }
        }

        CREDENTIALS
            .set(Some(creds))
            .map_err(|_e| CredentialsBootstrapError::AlreadyInitialized)?;
        Ok(CREDENTIALS
            .get()
            .ok_or(CredentialsBootstrapError::NotInitialized)?
            .as_ref())
    }

    async fn validate_with_api(creds: &CloudCredentials) -> CloudResult<()> {
        let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;
        client.get_user().await?;
        tracing::debug!("Cloud credentials validated with API");
        Ok(())
    }

    fn validation_is_fresh(creds: &CloudCredentials) -> bool {
        let Some(last) = creds.last_validated_at else {
            return false;
        };
        if creds.expires_within(Duration::hours(1)) {
            return false;
        }
        let age = Utc::now().signed_duration_since(last);
        age >= Duration::zero()
            && age < Duration::seconds(crate::constants::credentials::VALIDATION_TTL_SECS)
    }

    fn is_fly_container() -> bool {
        std::env::var("FLY_APP_NAME").is_ok()
    }

    fn allow_unvalidated() -> bool {
        std::env::var("SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS").as_deref() == Ok("1")
    }

    fn load_from_env() -> Option<CloudCredentials> {
        let api_token = read_env_optional("SYSTEMPROMPT_API_TOKEN")?;
        let user_email = read_env_optional("SYSTEMPROMPT_USER_EMAIL")?;

        tracing::debug!("Loading cloud credentials from environment variables");

        Some(CloudCredentials {
            api_token,
            api_url: read_env_optional("SYSTEMPROMPT_API_URL")
                .unwrap_or_else(|| crate::constants::api::PRODUCTION_URL.into()),
            authenticated_at: Utc::now(),
            user_email,
            last_validated_at: None,
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

    #[must_use]
    pub fn is_initialized() -> bool {
        CREDENTIALS.get().is_some()
    }

    pub fn init_empty() {
        if CREDENTIALS.set(None).is_err() {
            tracing::debug!("Credentials cell already initialised; init_empty is a no-op");
        }
    }

    pub async fn try_init() -> CloudResult<Option<&'static CloudCredentials>> {
        if CREDENTIALS.get().is_some() {
            return Self::get().map_err(Into::into);
        }
        Self::init().await
    }

    #[must_use]
    pub fn expires_within(duration: Duration) -> bool {
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
        let cloud_paths = crate::paths::get_cloud_paths();
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

    fn load_credentials_from_path(path: &Path) -> CloudResult<CloudCredentials> {
        let creds = CloudCredentials::load_from_path(path).map_err(|e| {
            if path.exists() {
                CloudError::from(CredentialsBootstrapError::InvalidCredentials {
                    message: e.to_string(),
                })
            } else {
                CloudError::from(CredentialsBootstrapError::FileNotFound {
                    path: path.display().to_string(),
                })
            }
        })?;

        if creds.is_token_expired() {
            return Err(CredentialsBootstrapError::TokenExpired.into());
        }

        if creds.expires_within(Duration::hours(1)) {
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

fn read_env_optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) | Err(_) => None,
    }
}
