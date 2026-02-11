use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use systemprompt_models::modules::ApiPaths;

use super::types::{
    ActivityRequest, ApiError, CheckoutRequest, CheckoutResponse, ListResponse, Plan, Tenant,
    UserMeResponse,
};

#[derive(Debug)]
pub struct CloudApiClient {
    pub(super) client: Client,
    pub(super) api_url: String,
    pub(super) token: String,
}

impl CloudApiClient {
    pub fn new(api_url: &str, token: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()?,
            api_url: api_url.to_string(),
            token: token.to_string(),
        })
    }

    #[must_use]
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    pub(super) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context("Failed to connect to API")?;

        self.handle_response(response).await
    }

    pub(super) async fn post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
            .context("Failed to connect to API")?;

        self.handle_response(response).await
    }

    pub(super) async fn post_no_response<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
            .context("Failed to connect to API")?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(anyhow!(
                "Authentication failed. Please run 'systemprompt cloud login' again."
            ));
        }
        if status == StatusCode::NO_CONTENT || status.is_success() {
            return Ok(());
        }

        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<failed to read response body>"));

        serde_json::from_str::<ApiError>(&error_text).map_or_else(
            |_| {
                Err(anyhow!(
                    "Request failed with status {}: {}",
                    status,
                    error_text.chars().take(500).collect::<String>()
                ))
            },
            |parsed| Err(anyhow!("{}: {}", parsed.error.code, parsed.error.message)),
        )
    }

    pub(super) async fn put<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
            .context("Failed to connect to API")?;

        self.handle_response(response).await
    }

    pub(super) async fn put_no_content<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
            .context("Failed to connect to API")?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(anyhow!(
                "Authentication failed. Please run 'systemprompt cloud login' again."
            ));
        }
        if status == StatusCode::NO_CONTENT || status.is_success() {
            return Ok(());
        }

        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<failed to read response body>"));

        serde_json::from_str::<ApiError>(&error_text).map_or_else(
            |_| {
                Err(anyhow!(
                    "Request failed with status {}: {}",
                    status,
                    error_text.chars().take(500).collect::<String>()
                ))
            },
            |parsed| Err(anyhow!("{}: {}", parsed.error.code, parsed.error.message)),
        )
    }

    pub(super) async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context("Failed to connect to API")?;

        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(anyhow!(
                "Authentication failed. Please run 'systemprompt cloud login' again."
            ));
        }

        if status == StatusCode::NO_CONTENT {
            return Ok(());
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<failed to read response body>"));

            return serde_json::from_str::<ApiError>(&error_text).map_or_else(
                |_| {
                    Err(anyhow!(
                        "Request failed with status {}: {}",
                        status,
                        error_text.chars().take(500).collect::<String>()
                    ))
                },
                |parsed| Err(anyhow!("{}: {}", parsed.error.code, parsed.error.message)),
            );
        }

        Ok(())
    }

    pub(super) async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context("Failed to connect to API")?;

        self.handle_response(response).await
    }

    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(anyhow!(
                "Authentication failed. Please run 'systemprompt cloud login' again."
            ));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<failed to read response body>"));

            return serde_json::from_str::<ApiError>(&error_text).map_or_else(
                |_| {
                    Err(anyhow!(
                        "Request failed with status {}: {}",
                        status,
                        error_text.chars().take(500).collect::<String>()
                    ))
                },
                |parsed| Err(anyhow!("{}: {}", parsed.error.code, parsed.error.message)),
            );
        }

        response
            .json()
            .await
            .context("Failed to parse API response")
    }

    pub async fn get_user(&self) -> Result<UserMeResponse> {
        self.get(ApiPaths::AUTH_ME).await
    }

    pub async fn list_tenants(&self) -> Result<Vec<Tenant>> {
        let response: ListResponse<Tenant> = self.get(ApiPaths::CLOUD_TENANTS).await?;
        Ok(response.data)
    }

    pub async fn get_plans(&self) -> Result<Vec<Plan>> {
        let plans: Vec<Plan> = self.get(ApiPaths::CLOUD_CHECKOUT_PLANS).await?;
        Ok(plans)
    }

    pub async fn create_checkout(
        &self,
        price_id: &str,
        region: &str,
        redirect_uri: Option<&str>,
    ) -> Result<CheckoutResponse> {
        let request = CheckoutRequest {
            price_id: price_id.to_string(),
            region: region.to_string(),
            redirect_uri: redirect_uri.map(String::from),
        };
        self.post(ApiPaths::CLOUD_CHECKOUT, &request).await
    }

    pub async fn report_activity(&self, event_type: &str, user_id: &str) -> Result<()> {
        let request = ActivityRequest {
            event: event_type.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            data: super::types::ActivityData {
                user_id: user_id.to_string(),
            },
        };
        self.post_no_response(ApiPaths::CLOUD_ACTIVITY, &request)
            .await
    }
}
