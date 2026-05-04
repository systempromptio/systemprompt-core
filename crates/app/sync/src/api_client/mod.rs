//! HTTP client used by sync push/pull/deploy.
//!
//! Handles direct-sync vs. cloud-relay endpoint selection, bearer-token
//! auth, retryable failures with exponential backoff, and typed JSON /
//! binary response handling.

mod response;
mod retry;

use std::time::Duration;

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_SYNC_DEPLOY_TIMEOUT};
use tokio::time::sleep;

use crate::error::{SyncError, SyncResult};
pub use retry::RetryConfig;

/// HTTP client for the systemprompt cloud sync API.
///
/// Wraps a `reqwest::Client` plus the bearer token, optional direct-sync
/// hostname/token, and a [`RetryConfig`] for exponential-backoff retries on
/// upload / download failures.
#[derive(Clone, Debug)]
pub struct SyncApiClient {
    client: Client,
    api_url: String,
    token: String,
    hostname: Option<String>,
    sync_token: Option<String>,
    retry_config: RetryConfig,
}

/// Registry credentials returned by the cloud API for `docker login`.
#[derive(Debug, Deserialize)]
pub struct RegistryToken {
    /// Registry hostname.
    pub registry: String,
    /// Username to log in as.
    pub username: String,
    /// Bearer token used as the registry password.
    pub token: String,
}

/// Response body returned by the cloud sync upload endpoint.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct UploadResponse {
    /// Number of files the server reports as uploaded.
    pub files_uploaded: usize,
}

/// Response body returned by the cloud deploy endpoint.
#[derive(Debug, Deserialize)]
pub struct DeployResponse {
    /// Deployment status string (`pending`, `running`, …).
    pub status: String,
    /// Public URL of the deployed app, when known.
    pub app_url: Option<String>,
}

impl SyncApiClient {
    /// Construct a new client targeting `api_url` with the given bearer
    /// token and the default [`RetryConfig`].
    pub fn new(api_url: &str, token: &str) -> SyncResult<Self> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_SYNC_DEPLOY_TIMEOUT)
                .build()?,
            api_url: api_url.to_string(),
            token: token.to_string(),
            hostname: None,
            sync_token: None,
            retry_config: RetryConfig::default(),
        })
    }

    /// Switch the client to direct-sync mode, talking straight to the
    /// supplied tenant hostname with `sync_token` instead of routing through
    /// the central cloud API. When either argument is `None`, the client
    /// keeps the cloud-relay endpoint.
    pub fn with_direct_sync(
        mut self,
        hostname: Option<String>,
        sync_token: Option<String>,
    ) -> Self {
        self.hostname = hostname;
        self.sync_token = sync_token;
        self
    }

    fn direct_sync_credentials(&self) -> Option<(String, String)> {
        match (&self.hostname, &self.sync_token) {
            (Some(hostname), Some(token)) => {
                let url = format!("https://{}/api/v1/sync/files", hostname);
                Some((url, token.clone()))
            },
            _ => None,
        }
    }

    fn calculate_next_delay(&self, current: Duration) -> Duration {
        self.retry_config.next_delay(current)
    }

    /// Upload the supplied bundle of files for `tenant_id` with retries.
    pub async fn upload_files(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
        data: Vec<u8>,
    ) -> SyncResult<UploadResponse> {
        let (url, token) = self.direct_sync_credentials().unwrap_or_else(|| {
            (
                format!("{}/api/v1/cloud/tenants/{}/files", self.api_url, tenant_id),
                self.token.clone(),
            )
        });

        let mut current_delay = self.retry_config.initial_delay;

        for attempt in 1..=self.retry_config.max_attempts {
            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/octet-stream")
                .body(data.clone())
                .send()
                .await?;

            match response::handle_json::<UploadResponse>(response).await {
                Ok(upload) => return Ok(upload),
                Err(error) if error.is_retryable() && attempt < self.retry_config.max_attempts => {
                    tracing::warn!(
                        attempt = attempt,
                        max_attempts = self.retry_config.max_attempts,
                        delay_ms = current_delay.as_millis() as u64,
                        error = %error,
                        "Retryable sync error, waiting before retry"
                    );
                    sleep(current_delay).await;
                    current_delay = self.calculate_next_delay(current_delay);
                },
                Err(error) => return Err(error),
            }
        }

        Err(SyncError::ApiError {
            status: 503,
            message: "Max retry attempts exceeded".to_string(),
        })
    }

    /// Download the file bundle for `tenant_id` with retries.
    pub async fn download_files(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
    ) -> SyncResult<Vec<u8>> {
        let (url, token) = self.direct_sync_credentials().unwrap_or_else(|| {
            (
                format!("{}/api/v1/cloud/tenants/{}/files", self.api_url, tenant_id),
                self.token.clone(),
            )
        });

        let mut current_delay = self.retry_config.initial_delay;

        for attempt in 1..=self.retry_config.max_attempts {
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;

            match response::handle_binary(response).await {
                Ok(data) => return Ok(data),
                Err(error) if error.is_retryable() && attempt < self.retry_config.max_attempts => {
                    tracing::warn!(
                        attempt = attempt,
                        max_attempts = self.retry_config.max_attempts,
                        delay_ms = current_delay.as_millis() as u64,
                        error = %error,
                        "Retryable sync error, waiting before retry"
                    );
                    sleep(current_delay).await;
                    current_delay = self.calculate_next_delay(current_delay);
                },
                Err(error) => return Err(error),
            }
        }

        Err(SyncError::ApiError {
            status: 503,
            message: "Max retry attempts exceeded".to_string(),
        })
    }

    /// Fetch transient registry credentials for `tenant_id`, used to
    /// `docker login` before pushing a deploy image.
    pub async fn get_registry_token(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
    ) -> SyncResult<RegistryToken> {
        let url = format!(
            "{}/api/v1/cloud/tenants/{}/registry-token",
            self.api_url, tenant_id
        );
        self.get(&url).await
    }

    /// Trigger a deploy for `tenant_id` using the supplied container image
    /// reference.
    pub async fn deploy(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
        image: &str,
    ) -> SyncResult<DeployResponse> {
        let url = format!("{}/api/v1/cloud/tenants/{}/deploy", self.api_url, tenant_id);
        self.post(&url, &serde_json::json!({ "image": image }))
            .await
    }

    /// Fetch the Fly.io app name associated with `tenant_id`. Returns
    /// [`SyncError::TenantNoApp`] if the tenant has no app configured.
    pub async fn get_tenant_app_id(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
    ) -> SyncResult<String> {
        #[derive(Deserialize)]
        struct TenantInfo {
            fly_app_name: Option<String>,
        }
        let url = format!("{}/api/v1/cloud/tenants/{}", self.api_url, tenant_id);
        let info: TenantInfo = self.get(&url).await?;
        info.fly_app_name.ok_or(SyncError::TenantNoApp)
    }

    /// Fetch the cloud database connection string for `tenant_id`. Returns
    /// a 404 [`SyncError::ApiError`] if no database is provisioned.
    pub async fn get_database_url(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
    ) -> SyncResult<String> {
        #[derive(Deserialize)]
        struct DatabaseInfo {
            database_url: Option<String>,
        }
        let url = format!(
            "{}/api/v1/cloud/tenants/{}/database",
            self.api_url, tenant_id
        );
        let info: DatabaseInfo = self.get(&url).await?;
        info.database_url.ok_or_else(|| SyncError::ApiError {
            status: 404,
            message: "Database URL not available for tenant".to_string(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, url: &str) -> SyncResult<T> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        response::handle_json(resp).await
    }

    async fn post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        url: &str,
        body: &B,
    ) -> SyncResult<T> {
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        response::handle_json(resp).await
    }
}
