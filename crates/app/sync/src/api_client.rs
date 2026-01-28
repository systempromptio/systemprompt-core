use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::error::{SyncError, SyncResult};

#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            exponential_base: 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyncApiClient {
    client: Client,
    api_url: String,
    token: String,
    hostname: Option<String>,
    sync_token: Option<String>,
    retry_config: RetryConfig,
}

#[derive(Debug, Deserialize)]
pub struct RegistryToken {
    pub registry: String,
    pub username: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct DeployResponse {
    pub status: String,
    pub app_url: Option<String>,
}

impl SyncApiClient {
    pub fn new(api_url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.to_string(),
            token: token.to_string(),
            hostname: None,
            sync_token: None,
            retry_config: RetryConfig::default(),
        }
    }

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
        current
            .saturating_mul(self.retry_config.exponential_base)
            .min(self.retry_config.max_delay)
    }

    pub async fn upload_files(&self, tenant_id: &str, data: Vec<u8>) -> SyncResult<()> {
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

            match self.handle_empty_response(response).await {
                Ok(()) => return Ok(()),
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
                }
                Err(error) => return Err(error),
            }
        }

        Err(SyncError::ApiError {
            status: 503,
            message: "Max retry attempts exceeded".to_string(),
        })
    }

    pub async fn download_files(&self, tenant_id: &str) -> SyncResult<Vec<u8>> {
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

            match self.handle_binary_response(response).await {
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
                }
                Err(error) => return Err(error),
            }
        }

        Err(SyncError::ApiError {
            status: 503,
            message: "Max retry attempts exceeded".to_string(),
        })
    }

    pub async fn get_registry_token(&self, tenant_id: &str) -> SyncResult<RegistryToken> {
        let url = format!(
            "{}/api/v1/cloud/tenants/{}/registry-token",
            self.api_url, tenant_id
        );
        self.get(&url).await
    }

    pub async fn deploy(&self, tenant_id: &str, image: &str) -> SyncResult<DeployResponse> {
        let url = format!("{}/api/v1/cloud/tenants/{}/deploy", self.api_url, tenant_id);
        self.post(&url, &serde_json::json!({ "image": image }))
            .await
    }

    pub async fn get_tenant_app_id(&self, tenant_id: &str) -> SyncResult<String> {
        #[derive(Deserialize)]
        struct TenantInfo {
            fly_app_name: Option<String>,
        }
        let url = format!("{}/api/v1/cloud/tenants/{}", self.api_url, tenant_id);
        let info: TenantInfo = self.get(&url).await?;
        info.fly_app_name.ok_or(SyncError::TenantNoApp)
    }

    pub async fn get_database_url(&self, tenant_id: &str) -> SyncResult<String> {
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
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        self.handle_json_response(response).await
    }

    async fn post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        url: &str,
        body: &B,
    ) -> SyncResult<T> {
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;

        self.handle_json_response(response).await
    }

    async fn handle_json_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> SyncResult<T> {
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(SyncError::Unauthorized);
        }
        if !status.is_success() {
            let message = response.text().await?;
            return Err(SyncError::ApiError {
                status: status.as_u16(),
                message,
            });
        }
        Ok(response.json().await?)
    }

    async fn handle_empty_response(&self, response: reqwest::Response) -> SyncResult<()> {
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await?;
            return Err(SyncError::ApiError {
                status: status.as_u16(),
                message,
            });
        }
        Ok(())
    }

    async fn handle_binary_response(&self, response: reqwest::Response) -> SyncResult<Vec<u8>> {
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|e| format!("(body unreadable: {})", e));
            return Err(SyncError::ApiError {
                status: status.as_u16(),
                message,
            });
        }
        Ok(response.bytes().await?.to_vec())
    }
}
