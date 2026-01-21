use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use systemprompt_models::modules::ApiPaths;

use super::types::{
    ApiError, ApiErrorDetail, ApiResponse, CheckoutRequest, CheckoutResponse, ListResponse, Plan,
    Tenant, UserMeResponse,
};

#[derive(Debug)]
pub struct CloudApiClient {
    pub(super) client: Client,
    pub(super) api_url: String,
    pub(super) token: String,
}

impl CloudApiClient {
    #[must_use]
    pub fn new(api_url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.to_string(),
            token: token.to_string(),
        }
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
        let error: ApiError = response.json().await.unwrap_or_else(|_| ApiError {
            error: ApiErrorDetail {
                code: "unknown".to_string(),
                message: format!("Request failed with status {status}"),
            },
        });
        Err(anyhow!("{}: {}", error.error.code, error.error.message))
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
            let error: ApiError = response.json().await.unwrap_or_else(|_| ApiError {
                error: ApiErrorDetail {
                    code: "unknown".to_string(),
                    message: format!("Request failed with status {status}"),
                },
            });
            return Err(anyhow!("{}: {}", error.error.code, error.error.message));
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
            let error: ApiError = response.json().await.unwrap_or_else(|_| ApiError {
                error: ApiErrorDetail {
                    code: "unknown".to_string(),
                    message: format!("Request failed with status {status}"),
                },
            });
            return Err(anyhow!("{}: {}", error.error.code, error.error.message));
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
}
