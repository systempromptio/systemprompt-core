//! Tenant-scoped endpoints for [`super::CloudApiClient`].

use serde::Serialize;
use systemprompt_identifiers::TenantId;
use systemprompt_models::modules::ApiPaths;

use super::CloudApiClient;
use super::types::{
    ApiResponse, CustomDomainResponse, DeployResponse, ExternalDbAccessResponse,
    ListSecretsResponse, RegistryToken, RotateCredentialsResponse, RotateSyncTokenResponse,
    SetCustomDomainRequest, SetExternalDbAccessRequest, SetSecretsRequest, StatusResponse,
    TenantSecrets, TenantStatus,
};
use crate::error::CloudResult;

#[derive(Serialize)]
struct DeployRequest {
    image: String,
}

impl CloudApiClient {
    pub async fn get_tenant_status(&self, tenant_id: &TenantId) -> CloudResult<TenantStatus> {
        let response: ApiResponse<TenantStatus> = self
            .get(&ApiPaths::tenant_status(tenant_id.as_str()))
            .await?;
        Ok(response.data)
    }

    pub async fn get_registry_token(&self, tenant_id: &TenantId) -> CloudResult<RegistryToken> {
        let response: ApiResponse<RegistryToken> = self
            .get(&ApiPaths::tenant_registry_token(tenant_id.as_str()))
            .await?;
        Ok(response.data)
    }

    pub async fn deploy(&self, tenant_id: &TenantId, image: &str) -> CloudResult<DeployResponse> {
        let request = DeployRequest {
            image: image.to_string(),
        };
        let response: ApiResponse<DeployResponse> = self
            .post(&ApiPaths::tenant_deploy(tenant_id.as_str()), &request)
            .await?;
        Ok(response.data)
    }

    pub async fn fetch_secrets(&self, secrets_url: &str) -> CloudResult<TenantSecrets> {
        let path = secrets_url
            .strip_prefix(&self.api_url)
            .unwrap_or(secrets_url);
        self.get(path).await
    }

    pub async fn delete_tenant(&self, tenant_id: &TenantId) -> CloudResult<()> {
        self.delete(&ApiPaths::tenant(tenant_id.as_str())).await
    }

    pub async fn restart_tenant(&self, tenant_id: &TenantId) -> CloudResult<StatusResponse> {
        self.post_empty(&ApiPaths::tenant_restart(tenant_id.as_str()))
            .await
    }

    pub async fn retry_provision(&self, tenant_id: &TenantId) -> CloudResult<StatusResponse> {
        self.post_empty(&ApiPaths::tenant_retry_provision(tenant_id.as_str()))
            .await
    }

    pub async fn set_secrets(
        &self,
        tenant_id: &TenantId,
        secrets: std::collections::HashMap<String, String>,
    ) -> CloudResult<Vec<String>> {
        let keys: Vec<String> = secrets.keys().cloned().collect();
        let request = SetSecretsRequest { secrets };
        self.put_no_content(&ApiPaths::tenant_secrets(tenant_id.as_str()), &request)
            .await?;
        Ok(keys)
    }

    pub async fn unset_secret(&self, tenant_id: &TenantId, key: &str) -> CloudResult<()> {
        let path = format!("{}/{}", ApiPaths::tenant_secrets(tenant_id.as_str()), key);
        self.delete(&path).await
    }

    pub async fn set_external_db_access(
        &self,
        tenant_id: &TenantId,
        enabled: bool,
    ) -> CloudResult<ExternalDbAccessResponse> {
        let request = SetExternalDbAccessRequest { enabled };
        let response: ApiResponse<ExternalDbAccessResponse> = self
            .put(
                &ApiPaths::tenant_external_db_access(tenant_id.as_str()),
                &request,
            )
            .await?;
        Ok(response.data)
    }

    pub async fn rotate_credentials(
        &self,
        tenant_id: &TenantId,
    ) -> CloudResult<RotateCredentialsResponse> {
        self.post_empty(&ApiPaths::tenant_rotate_credentials(tenant_id.as_str()))
            .await
    }

    pub async fn rotate_sync_token(
        &self,
        tenant_id: &TenantId,
    ) -> CloudResult<RotateSyncTokenResponse> {
        let response: ApiResponse<RotateSyncTokenResponse> = self
            .post_empty(&ApiPaths::tenant_rotate_sync_token(tenant_id.as_str()))
            .await?;
        Ok(response.data)
    }

    pub async fn list_secrets(&self, tenant_id: &TenantId) -> CloudResult<ListSecretsResponse> {
        self.get(&ApiPaths::tenant_secrets(tenant_id.as_str())).await
    }

    pub async fn set_custom_domain(
        &self,
        tenant_id: &TenantId,
        domain: &str,
    ) -> CloudResult<CustomDomainResponse> {
        let request = SetCustomDomainRequest {
            domain: domain.to_string(),
        };
        let response: ApiResponse<CustomDomainResponse> = self
            .post(
                &ApiPaths::tenant_custom_domain(tenant_id.as_str()),
                &request,
            )
            .await?;
        Ok(response.data)
    }

    pub async fn get_custom_domain(
        &self,
        tenant_id: &TenantId,
    ) -> CloudResult<CustomDomainResponse> {
        let response: ApiResponse<CustomDomainResponse> = self
            .get(&ApiPaths::tenant_custom_domain(tenant_id.as_str()))
            .await?;
        Ok(response.data)
    }

    pub async fn delete_custom_domain(&self, tenant_id: &TenantId) -> CloudResult<()> {
        self.delete(&ApiPaths::tenant_custom_domain(tenant_id.as_str()))
            .await
    }

    pub async fn cancel_subscription(&self, tenant_id: &TenantId) -> CloudResult<()> {
        self.post_empty(&ApiPaths::tenant_subscription_cancel(tenant_id.as_str()))
            .await
    }
}
