//! Cloud credentials bootstrap: loads and validates the operator's cloud
//! credentials and hands them to the caller, which owns them from there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;

use std::path::Path;

use chrono::{Duration, Utc};
use systemprompt_identifiers::{CloudAuthToken, Email};
use systemprompt_manifest::read_env_optional;

pub use error::CredentialsBootstrapError;

use crate::error::{CloudError, CloudResult};
use crate::{CloudApiClient, CloudCredentials};

const POD_CREDENTIALS_REJECTED: &str = "tenant pod credentials rejected by api.systemprompt.io \
                                        (token in SYSTEMPROMPT_API_TOKEN). Re-run 'systemprompt \
                                        cloud deploy' or set \
                                        SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1 to bypass";

#[derive(Debug, Clone, Copy)]
pub struct CredentialsBootstrap;

impl CredentialsBootstrap {
    pub async fn load() -> CloudResult<Option<CloudCredentials>> {
        if Self::is_deployment_host() {
            tracing::debug!("Deployment host detected, loading credentials from environment");
            let creds = Self::load_from_env();
            if let Some(ref c) = creds
                && let Err(e) = Self::validate_with_api(c).await
            {
                if Self::allow_unvalidated() {
                    tracing::warn!(
                        target: "security_audit",
                        error = %e,
                        "cloud credentials unvalidated; proceeding under SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1"
                    );
                } else {
                    return Err(CredentialsBootstrapError::ApiValidationFailed {
                        message: POD_CREDENTIALS_REJECTED.to_owned(),
                        source: Box::new(e),
                    }
                    .into());
                }
            }
            return Ok(creds);
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

        Ok(Some(creds))
    }

    async fn validate_with_api(creds: &CloudCredentials) -> CloudResult<()> {
        let client = CloudApiClient::new(&creds.api_url, creds.api_token.as_str())?;
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

    fn is_deployment_host() -> bool {
        systemprompt_models::subprocess::is_deployment_host(|name| std::env::var(name).ok())
    }

    fn allow_unvalidated() -> bool {
        std::env::var("SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS").as_deref() == Ok("1")
    }

    fn load_from_env() -> Option<CloudCredentials> {
        let api_token = CloudAuthToken::new(read_env_optional("SYSTEMPROMPT_API_TOKEN")?);
        let user_email = match Email::try_new(read_env_optional("SYSTEMPROMPT_USER_EMAIL")?) {
            Ok(email) => email,
            Err(error) => {
                tracing::warn!(error = %error, "SYSTEMPROMPT_USER_EMAIL is not a valid address");
                return None;
            },
        };

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

    pub fn read_unvalidated() -> CloudResult<Option<CloudCredentials>> {
        if Self::is_deployment_host() {
            return Ok(Self::load_from_env());
        }
        let cloud_paths = crate::paths::get_cloud_paths();
        let credentials_path = cloud_paths.resolve(crate::paths::CloudPath::Credentials);
        Self::load_credentials_from_path(&credentials_path).map(Some)
    }

    pub async fn require() -> CloudResult<CloudCredentials> {
        Self::load()
            .await?
            .ok_or_else(|| CredentialsBootstrapError::NotAvailable.into())
    }

    pub async fn reload() -> Result<CloudCredentials, CredentialsBootstrapError> {
        let cloud_paths = crate::paths::get_cloud_paths();
        let credentials_path = cloud_paths.resolve(crate::paths::CloudPath::Credentials);

        let creds = Self::load_credentials_from_path(&credentials_path).map_err(|e| {
            CredentialsBootstrapError::InvalidCredentials {
                source: Box::new(e),
            }
        })?;

        Self::validate_with_api(&creds).await.map_err(|e| {
            CredentialsBootstrapError::ApiValidationFailed {
                message: "credentials rejected by the cloud API".to_owned(),
                source: Box::new(e),
            }
        })?;

        Ok(creds)
    }

    fn load_credentials_from_path(path: &Path) -> CloudResult<CloudCredentials> {
        let creds = CloudCredentials::load_from_path(path).map_err(|e| {
            if path.exists() {
                CloudError::from(CredentialsBootstrapError::InvalidCredentials {
                    source: Box::new(e),
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
                "Cloud token will expire soon. Consider running 'systemprompt cloud auth login' to \
                 refresh."
            );
        }

        tracing::debug!(path = %path.display(), user = ?creds.user_email, "Loaded cloud credentials");

        Ok(creds)
    }
}
