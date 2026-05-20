//! Low-level HTTP verb helpers for [`super::CloudApiClient`].
//!
//! Two flavours live here:
//!
//! - `get` / `post` / `post_no_response` send the operator JWT (`self.token`)
//!   verbatim as a bearer. These are used by control-plane endpoints in
//!   `endpoints.rs`.
//! - `tenant_get` / `tenant_post` / `tenant_put` / `tenant_delete` /
//!   `tenant_post_empty` / `tenant_put_no_content` first acquire a short-lived
//!   access token via RFC 8693 token-exchange against the tenant deployment's
//!   `/api/v1/core/oauth/token` endpoint, cache it for the lifetime of this
//!   client, and retry exactly once after clearing the cache on a 401.

use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::CloudApiClient;
use crate::error::{CloudError, CloudResult};

const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(30);
const RFC8693_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const RFC8693_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";

#[derive(Debug, Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

impl CloudApiClient {
    pub(super) async fn get<T: DeserializeOwned>(&self, path: &str) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        self.handle_response(response).await
    }

    pub(super) async fn post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        self.handle_response(response).await
    }

    pub(super) async fn post_no_response<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        self.handle_no_content_response(response).await
    }

    async fn tenant_access_token(&self) -> CloudResult<String> {
        {
            let cached = self.tenant_token_cache.lock().await;
            if let Some((token, expires_at)) = cached.as_ref() {
                if *expires_at > Instant::now() + TOKEN_REFRESH_MARGIN {
                    return Ok(token.clone());
                }
            }
        }
        self.exchange_token().await
    }

    async fn exchange_token(&self) -> CloudResult<String> {
        let url = format!("{}/api/v1/core/oauth/token", self.api_url);
        let response = self
            .client
            .post(&url)
            .form(&[
                ("grant_type", RFC8693_GRANT_TYPE),
                ("subject_token", self.token.as_str()),
                ("subject_token_type", RFC8693_TOKEN_TYPE),
                ("resource", self.api_url.as_str()),
            ])
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthorized);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(CloudError::HttpStatus {
                status: status.as_u16(),
                body: body.chars().take(500).collect(),
            });
        }

        let parsed: TokenExchangeResponse = response.json().await?;
        let lifetime = parsed
            .expires_in
            .map_or_else(|| Duration::from_secs(300), Duration::from_secs);
        let expires_at = Instant::now() + lifetime;

        let access_token = parsed.access_token;
        {
            let mut cached = self.tenant_token_cache.lock().await;
            *cached = Some((access_token.clone(), expires_at));
        }
        Ok(access_token)
    }

    async fn invalidate_tenant_token(&self) {
        let mut cached = self.tenant_token_cache.lock().await;
        *cached = None;
    }

    pub(super) async fn tenant_get<T: DeserializeOwned>(&self, path: &str) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let send = || async {
            let bearer = self.tenant_access_token().await?;
            self.client
                .get(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .send()
                .await
                .map_err(CloudError::from)
        };
        let response = send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.invalidate_tenant_token().await;
            let retry = send().await?;
            return self.handle_response(retry).await;
        }
        self.handle_response(response).await
    }

    pub(super) async fn tenant_post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let send = || async {
            let bearer = self.tenant_access_token().await?;
            self.client
                .post(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .json(body)
                .send()
                .await
                .map_err(CloudError::from)
        };
        let response = send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.invalidate_tenant_token().await;
            let retry = send().await?;
            return self.handle_response(retry).await;
        }
        self.handle_response(response).await
    }

    pub(super) async fn tenant_put<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let send = || async {
            let bearer = self.tenant_access_token().await?;
            self.client
                .put(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .json(body)
                .send()
                .await
                .map_err(CloudError::from)
        };
        let response = send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.invalidate_tenant_token().await;
            let retry = send().await?;
            return self.handle_response(retry).await;
        }
        self.handle_response(response).await
    }

    pub(super) async fn tenant_put_no_content<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<()> {
        let url = format!("{}{}", self.api_url, path);
        let send = || async {
            let bearer = self.tenant_access_token().await?;
            self.client
                .put(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .json(body)
                .send()
                .await
                .map_err(CloudError::from)
        };
        let response = send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.invalidate_tenant_token().await;
            let retry = send().await?;
            return self.handle_no_content_response(retry).await;
        }
        self.handle_no_content_response(response).await
    }

    pub(super) async fn tenant_delete(&self, path: &str) -> CloudResult<()> {
        let url = format!("{}{}", self.api_url, path);
        let send = || async {
            let bearer = self.tenant_access_token().await?;
            self.client
                .delete(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .send()
                .await
                .map_err(CloudError::from)
        };
        let response = send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.invalidate_tenant_token().await;
            let retry = send().await?;
            return self.handle_no_content_response(retry).await;
        }
        self.handle_no_content_response(response).await
    }

    pub(super) async fn tenant_post_empty<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let send = || async {
            let bearer = self.tenant_access_token().await?;
            self.client
                .post(&url)
                .header("Authorization", format!("Bearer {bearer}"))
                .send()
                .await
                .map_err(CloudError::from)
        };
        let response = send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.invalidate_tenant_token().await;
            let retry = send().await?;
            return self.handle_response(retry).await;
        }
        self.handle_response(response).await
    }
}
