use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::CloudAuthToken;
use systemprompt_logging::CliService;
use validator::Validate;

use crate::auth;
use crate::error::CloudError;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CloudCredentials {
    #[validate(length(min = 1, message = "API token cannot be empty"))]
    pub api_token: String,

    #[validate(url(message = "API URL must be a valid URL"))]
    pub api_url: String,

    pub authenticated_at: DateTime<Utc>,

    #[validate(email(message = "User email must be a valid email address"))]
    pub user_email: String,
}

impl CloudCredentials {
    #[must_use]
    pub fn new(api_token: String, api_url: String, user_email: String) -> Self {
        Self {
            api_token,
            api_url,
            authenticated_at: Utc::now(),
            user_email,
        }
    }

    pub fn token(&self) -> CloudAuthToken {
        CloudAuthToken::new(&self.api_token)
    }

    #[must_use]
    pub fn is_token_expired(&self) -> bool {
        auth::is_expired(&self.token())
    }

    #[must_use]
    pub fn expires_within(&self, duration: Duration) -> bool {
        auth::expires_within(&self.token(), duration)
    }

    pub fn load_and_validate_from_path(path: &Path) -> Result<Self> {
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
            return Err(CloudError::TokenExpired.into());
        }

        if creds.expires_within(Duration::hours(1)) {
            CliService::warning(
                "Cloud token will expire soon. Consider running 'systemprompt cloud login' to \
                 refresh.",
            );
        }

        Ok(creds)
    }

    pub async fn validate_with_api(&self) -> Result<bool> {
        let client = reqwest::Client::new();

        let response = client
            .get(format!("{}/api/v1/auth/me", self.api_url))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(CloudError::NotAuthenticated.into());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

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

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
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

    pub fn delete_from_path(path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
