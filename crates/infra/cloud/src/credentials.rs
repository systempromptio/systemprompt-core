//! On-disk representation of authenticated cloud credentials.

use std::fs;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::CloudAuthToken;
use systemprompt_logging::CliService;
use systemprompt_models::net::{HTTP_AUTH_VERIFY_TIMEOUT, HTTP_CONNECT_TIMEOUT};
use validator::Validate;

use crate::auth;
use crate::error::{CloudError, CloudResult};

/// Persisted CLI authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CloudCredentials {
    /// Bearer token issued by the cloud.
    #[validate(length(min = 1, message = "API token cannot be empty"))]
    pub api_token: String,

    /// Cloud API base URL the token was issued for.
    #[validate(url(message = "API URL must be a valid URL"))]
    pub api_url: String,

    /// Wall-clock time the credentials were saved.
    pub authenticated_at: DateTime<Utc>,

    /// Email of the authenticated user.
    #[validate(email(message = "User email must be a valid email address"))]
    pub user_email: String,
}

impl CloudCredentials {
    /// Construct a fresh credentials record with `authenticated_at`
    /// set to now.
    #[must_use]
    pub fn new(api_token: String, api_url: String, user_email: String) -> Self {
        Self {
            api_token,
            api_url,
            authenticated_at: Utc::now(),
            user_email,
        }
    }

    /// Wrap the persisted bearer token as a typed [`CloudAuthToken`].
    #[must_use]
    pub fn token(&self) -> CloudAuthToken {
        CloudAuthToken::new(&self.api_token)
    }

    /// `true` if the embedded JWT has expired.
    #[must_use]
    pub fn is_token_expired(&self) -> bool {
        auth::is_expired(&self.token())
    }

    /// `true` if the embedded JWT will expire within `duration`.
    #[must_use]
    pub fn expires_within(&self, duration: Duration) -> bool {
        auth::expires_within(&self.token(), duration)
    }

    /// Load and validate credentials at `path`, surfacing expiry as
    /// a typed error.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TokenExpired`] when the JWT is expired,
    /// [`CloudError::CredentialsCorrupted`] when validation fails, or
    /// [`CloudError::Io`] / [`CloudError::Json`] for I/O and parse
    /// failures.
    pub fn load_and_validate_from_path(path: &Path) -> CloudResult<Self> {
        let creds = Self::load_from_path(path)?;

        creds
            .validate()
            .map_err(|e| CloudError::CredentialsCorrupted {
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )),
            })?;

        if creds.is_token_expired() {
            return Err(CloudError::TokenExpired);
        }

        if creds.expires_within(Duration::hours(1)) {
            CliService::warning(
                "Cloud token will expire soon. Consider running 'systemprompt cloud login' to \
                 refresh.",
            );
        }

        Ok(creds)
    }

    /// Hit `GET /auth/me` to confirm the token is accepted by the
    /// cloud.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::Network`] for transport failures.
    pub async fn validate_with_api(&self) -> CloudResult<bool> {
        let client = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_AUTH_VERIFY_TIMEOUT)
            .build()?;

        let response = client
            .get(format!("{}/api/v1/auth/me", self.api_url))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// Read and parse credentials JSON at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::NotAuthenticated`] when the file does
    /// not exist, [`CloudError::Io`] for read failures, or
    /// [`CloudError::CredentialsCorrupted`] when validation fails.
    pub fn load_from_path(path: &Path) -> CloudResult<Self> {
        if !path.exists() {
            return Err(CloudError::NotAuthenticated);
        }

        let content = fs::read_to_string(path)?;

        let creds: Self = serde_json::from_str(&content)
            .map_err(|e| CloudError::CredentialsCorrupted { source: e })?;

        creds
            .validate()
            .map_err(|e| CloudError::CredentialsCorrupted {
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )),
            })?;

        Ok(creds)
    }

    /// Validate and write credentials to `path` with `0o600`
    /// permissions on Unix.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::CredentialsCorrupted`] when validation
    /// fails before writing, [`CloudError::Io`] for write failures,
    /// or [`CloudError::Json`] when serialization fails.
    pub fn save_to_path(&self, path: &Path) -> CloudResult<()> {
        self.validate()
            .map_err(|e| CloudError::CredentialsCorrupted {
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )),
            })?;

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

    /// Remove the credentials file at `path` (no-op if absent).
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::Io`] on filesystem failures.
    pub fn delete_from_path(path: &Path) -> CloudResult<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
