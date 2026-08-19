//! Top-level API endpoints not specific to tenants.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::modules::ApiPaths;

use super::CloudApiClient;
use super::types::{ListResponse, Tenant, UserMeResponse};
use crate::error::CloudResult;

impl CloudApiClient {
    pub async fn get_user(&self) -> CloudResult<UserMeResponse> {
        self.get(ApiPaths::AUTH_ME).await
    }

    pub async fn list_tenants(&self) -> CloudResult<Vec<Tenant>> {
        let response: ListResponse<Tenant> = self.get(ApiPaths::CLOUD_TENANTS).await?;
        Ok(response.data)
    }
}
