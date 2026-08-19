//! Tenant-scoped endpoints for [`super::CloudApiClient`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::TenantId;
use systemprompt_models::modules::ApiPaths;

use super::CloudApiClient;
use super::types::{
    ApiResponse, DeployResponse, RegistryToken, RotateCredentialsResponse, SetSecretsRequest,
    TenantSecrets, TenantStatus,
};
use crate::error::CloudResult;

#[derive(Serialize)]
struct DeployRequest {
    image: String,
}

impl CloudApiClient {
    pub async fn get_tenant_status(&self, tenant_id: &TenantId) -> CloudResult<TenantStatus> {
        let response: ApiResponse<TenantStatus> =
            self.tenant_get(&ApiPaths::tenant_status(tenant_id)).await?;
        Ok(response.data)
    }

    pub async fn get_registry_token(&self, tenant_id: &TenantId) -> CloudResult<RegistryToken> {
        let response: ApiResponse<RegistryToken> = self
            .tenant_get(&ApiPaths::tenant_registry_token(tenant_id))
            .await?;
        Ok(response.data)
    }

    pub async fn deploy(&self, tenant_id: &TenantId, image: &str) -> CloudResult<DeployResponse> {
        let request = DeployRequest {
            image: image.to_owned(),
        };
        let response: ApiResponse<DeployResponse> = self
            .tenant_post(&ApiPaths::tenant_deploy(tenant_id), &request)
            .await?;
        Ok(response.data)
    }

    pub async fn fetch_secrets(&self, secrets_url: &str) -> CloudResult<TenantSecrets> {
        let path = secrets_url
            .strip_prefix(&self.api_url)
            .unwrap_or(secrets_url);
        self.tenant_get(path).await
    }

    pub async fn delete_tenant(&self, tenant_id: &TenantId) -> CloudResult<()> {
        self.tenant_delete(&ApiPaths::tenant(tenant_id)).await
    }

    pub async fn set_secrets(
        &self,
        tenant_id: &TenantId,
        secrets: std::collections::HashMap<String, String>,
    ) -> CloudResult<Vec<String>> {
        let keys: Vec<String> = secrets.keys().cloned().collect();
        let request = SetSecretsRequest { secrets };
        self.tenant_put_no_content(&ApiPaths::tenant_secrets(tenant_id), &request)
            .await?;
        Ok(keys)
    }

    pub async fn rotate_credentials(
        &self,
        tenant_id: &TenantId,
    ) -> CloudResult<RotateCredentialsResponse> {
        self.tenant_post_empty(&ApiPaths::tenant_rotate_credentials(tenant_id))
            .await
    }
}
