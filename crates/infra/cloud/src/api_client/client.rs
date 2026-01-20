use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use systemprompt_models::modules::ApiPaths;

use super::types::{
    ApiError, ApiErrorDetail, ApiResponse, CheckoutRequest, CheckoutResponse,
    CustomDomainResponse, DeployResponse, ExternalDbAccessResponse, ListResponse,
    ListSecretsResponse, Plan, RegistryToken, RotateCredentialsResponse, RotateSyncTokenResponse,
    SetCustomDomainRequest, SetExternalDbAccessRequest, SetSecretsRequest, StatusResponse, Tenant,
    TenantSecrets, TenantStatus, UserMeResponse,
};

#[derive(Serialize)]
struct DeployRequest {
    image: String,
}

#[derive(Debug)]
pub struct CloudApiClient {
    client: Client,
    api_url: String,
    token: String,
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

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
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

    async fn post<T: DeserializeOwned, B: Serialize + Sync>(
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

    async fn put<T: DeserializeOwned, B: Serialize + Sync>(
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

    async fn put_no_content<B: Serialize + Sync>(&self, path: &str, body: &B) -> Result<()> {
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

    async fn delete(&self, path: &str) -> Result<()> {
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

    async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
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

    pub async fn get_tenant_status(&self, tenant_id: &str) -> Result<TenantStatus> {
        let response: ApiResponse<TenantStatus> =
            self.get(&ApiPaths::tenant_status(tenant_id)).await?;
        Ok(response.data)
    }

    pub async fn get_registry_token(&self, tenant_id: &str) -> Result<RegistryToken> {
        let response: ApiResponse<RegistryToken> = self
            .get(&ApiPaths::tenant_registry_token(tenant_id))
            .await?;
        Ok(response.data)
    }

    pub async fn deploy(&self, tenant_id: &str, image: &str) -> Result<DeployResponse> {
        let request = DeployRequest {
            image: image.to_string(),
        };
        let response: ApiResponse<DeployResponse> = self
            .post(&ApiPaths::tenant_deploy(tenant_id), &request)
            .await?;
        Ok(response.data)
    }

    pub async fn fetch_secrets(&self, secrets_url: &str) -> Result<TenantSecrets> {
        let path = secrets_url
            .strip_prefix(&self.api_url)
            .unwrap_or(secrets_url);
        self.get(path).await
    }

    pub async fn delete_tenant(&self, tenant_id: &str) -> Result<()> {
        self.delete(&ApiPaths::tenant(tenant_id)).await
    }

    pub async fn restart_tenant(&self, tenant_id: &str) -> Result<StatusResponse> {
        self.post_empty(&ApiPaths::tenant_restart(tenant_id)).await
    }

    pub async fn retry_provision(&self, tenant_id: &str) -> Result<StatusResponse> {
        self.post_empty(&ApiPaths::tenant_retry_provision(tenant_id))
            .await
    }

    pub async fn set_secrets(
        &self,
        tenant_id: &str,
        secrets: std::collections::HashMap<String, String>,
    ) -> Result<Vec<String>> {
        let keys: Vec<String> = secrets.keys().cloned().collect();
        let request = SetSecretsRequest { secrets };
        self.put_no_content(&ApiPaths::tenant_secrets(tenant_id), &request)
            .await?;
        Ok(keys)
    }

    pub async fn unset_secret(&self, tenant_id: &str, key: &str) -> Result<()> {
        let path = format!("{}/{}", ApiPaths::tenant_secrets(tenant_id), key);
        self.delete(&path).await
    }

    pub async fn set_external_db_access(
        &self,
        tenant_id: &str,
        enabled: bool,
    ) -> Result<ExternalDbAccessResponse> {
        let request = SetExternalDbAccessRequest { enabled };
        let response: ApiResponse<ExternalDbAccessResponse> = self
            .put(&ApiPaths::tenant_external_db_access(tenant_id), &request)
            .await?;
        Ok(response.data)
    }

    pub async fn rotate_credentials(&self, tenant_id: &str) -> Result<RotateCredentialsResponse> {
        self.post_empty(&ApiPaths::tenant_rotate_credentials(tenant_id))
            .await
    }

    pub async fn rotate_sync_token(&self, tenant_id: &str) -> Result<RotateSyncTokenResponse> {
        self.post_empty(&ApiPaths::tenant_rotate_sync_token(tenant_id))
            .await
    }

    pub async fn list_secrets(&self, tenant_id: &str) -> Result<ListSecretsResponse> {
        self.get(&ApiPaths::tenant_secrets(tenant_id)).await
    }

    pub async fn set_custom_domain(
        &self,
        tenant_id: &str,
        domain: &str,
    ) -> Result<CustomDomainResponse> {
        let request = SetCustomDomainRequest {
            domain: domain.to_string(),
        };
        let response: ApiResponse<CustomDomainResponse> = self
            .post(&ApiPaths::tenant_custom_domain(tenant_id), &request)
            .await?;
        Ok(response.data)
    }

    pub async fn get_custom_domain(&self, tenant_id: &str) -> Result<CustomDomainResponse> {
        let response: ApiResponse<CustomDomainResponse> =
            self.get(&ApiPaths::tenant_custom_domain(tenant_id)).await?;
        Ok(response.data)
    }

    pub async fn delete_custom_domain(&self, tenant_id: &str) -> Result<()> {
        self.delete(&ApiPaths::tenant_custom_domain(tenant_id))
            .await
    }
}
