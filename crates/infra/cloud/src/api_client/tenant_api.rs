use anyhow::Result;
use serde::Serialize;
use systemprompt_models::modules::ApiPaths;

use super::types::{
    ApiResponse, CustomDomainResponse, DeployResponse, ExternalDbAccessResponse,
    ListSecretsResponse, RegistryToken, RotateCredentialsResponse, RotateSyncTokenResponse,
    SetCustomDomainRequest, SetExternalDbAccessRequest, SetSecretsRequest, StatusResponse,
    TenantSecrets, TenantStatus,
};
use super::CloudApiClient;

#[derive(Serialize)]
struct DeployRequest {
    image: String,
}

impl CloudApiClient {
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
