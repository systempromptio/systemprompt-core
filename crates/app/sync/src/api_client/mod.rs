//! HTTP client used by sync push/pull/deploy.
//!
//! Handles direct-sync vs. cloud-relay endpoint selection, bearer-token
//! auth, retryable failures with exponential backoff, and typed JSON /
//! binary response handling.

mod response;
mod retry;
mod token;

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_SYNC_DEPLOY_TIMEOUT};
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::error::{SyncError, SyncResult};
pub use retry::RetryConfig;
pub use token::{exchange_subject_token, exchange_subject_token_at};
use token::is_unauthorized;

#[derive(Clone, Debug)]
pub struct SyncApiClient {
    client: Client,
    api_url: String,
    token: String,
    direct_sync_origin: Option<String>,
    cached_sync_token: Arc<Mutex<Option<String>>>,
    retry_config: RetryConfig,
}

#[derive(Debug, Deserialize)]
pub struct RegistryToken {
    pub registry: String,
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct UploadResponse {
    pub files_uploaded: usize,
}

#[derive(Debug, Deserialize)]
pub struct DeployResponse {
    pub status: String,
    pub app_url: Option<String>,
}

impl SyncApiClient {
    pub fn new(api_url: &str, token: &str) -> SyncResult<Self> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_SYNC_DEPLOY_TIMEOUT)
                .build()?,
            api_url: api_url.to_owned(),
            token: token.to_owned(),
            direct_sync_origin: None,
            cached_sync_token: Arc::new(Mutex::new(None)),
            retry_config: RetryConfig::default(),
        })
    }

    pub fn with_direct_sync(self, hostname: Option<String>) -> Self {
        let origin = hostname.map(|h| format!("https://{h}"));
        self.with_direct_sync_origin(origin)
    }

    pub fn with_direct_sync_origin(mut self, origin: Option<String>) -> Self {
        self.direct_sync_origin = origin;
        self
    }

    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    pub(crate) fn direct_sync_origin(&self) -> Option<&str> {
        self.direct_sync_origin.as_deref()
    }

    const fn is_direct_sync(&self) -> bool {
        self.direct_sync_origin.is_some()
    }

    fn files_url(&self, tenant_id: &systemprompt_identifiers::TenantId) -> String {
        self.direct_sync_origin.as_ref().map_or_else(
            || format!("{}/api/v1/cloud/tenants/{}/files", self.api_url, tenant_id),
            |origin| format!("{origin}/api/v1/sync/files"),
        )
    }

    fn calculate_next_delay(&self, current: Duration) -> Duration {
        self.retry_config.next_delay(current)
    }

    pub async fn upload_files(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
        data: Vec<u8>,
    ) -> SyncResult<UploadResponse> {
        let url = self.files_url(tenant_id);
        let direct = self.is_direct_sync();
        let mut bearer = self.bearer_token(false).await?;
        let mut reminted = false;

        let mut current_delay = self.retry_config.initial_delay;

        for attempt in 1..=self.retry_config.max_attempts {
            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .header("Content-Type", "application/octet-stream")
                .body(data.clone())
                .send()
                .await?;

            match response::handle_json::<UploadResponse>(response).await {
                Ok(upload) => return Ok(upload),
                Err(error) if direct && !reminted && is_unauthorized(&error) => {
                    reminted = true;
                    bearer = self.bearer_token(true).await?;
                },
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
            message: "Max retry attempts exceeded".to_owned(),
        })
    }

    pub async fn download_files(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
    ) -> SyncResult<Vec<u8>> {
        let url = self.files_url(tenant_id);
        let direct = self.is_direct_sync();
        let mut bearer = self.bearer_token(false).await?;
        let mut reminted = false;

        let mut current_delay = self.retry_config.initial_delay;

        for attempt in 1..=self.retry_config.max_attempts {
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .send()
                .await?;

            match response::handle_binary(response).await {
                Ok(data) => return Ok(data),
                Err(error) if direct && !reminted && is_unauthorized(&error) => {
                    reminted = true;
                    bearer = self.bearer_token(true).await?;
                },
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
            message: "Max retry attempts exceeded".to_owned(),
        })
    }

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

    pub async fn deploy(
        &self,
        tenant_id: &systemprompt_identifiers::TenantId,
        image: &str,
    ) -> SyncResult<DeployResponse> {
        let url = format!("{}/api/v1/cloud/tenants/{}/deploy", self.api_url, tenant_id);
        self.post(&url, &serde_json::json!({ "image": image }))
            .await
    }

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
            message: "Database URL not available for tenant".to_owned(),
        })
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, url: &str) -> SyncResult<T> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        response::handle_json(resp).await
    }

    pub(crate) async fn post<T: DeserializeOwned, B: Serialize + Sync>(
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
