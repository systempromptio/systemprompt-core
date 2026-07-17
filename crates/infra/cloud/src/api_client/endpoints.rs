//! Top-level API endpoints not specific to tenants.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{PriceId, UserId};
use systemprompt_models::modules::ApiPaths;

use super::CloudApiClient;
use super::types::{
    ActivityData, ActivityRequest, CheckoutRequest, CheckoutResponse, ListResponse, Plan, Tenant,
    UserMeResponse,
};
use crate::error::CloudResult;

impl CloudApiClient {
    pub async fn get_user(&self) -> CloudResult<UserMeResponse> {
        self.get(ApiPaths::AUTH_ME).await
    }

    pub async fn list_tenants(&self) -> CloudResult<Vec<Tenant>> {
        let response: ListResponse<Tenant> = self.get(ApiPaths::CLOUD_TENANTS).await?;
        Ok(response.data)
    }

    pub async fn get_plans(&self) -> CloudResult<Vec<Plan>> {
        let plans: Vec<Plan> = self.get(ApiPaths::CLOUD_CHECKOUT_PLANS).await?;
        Ok(plans)
    }

    pub async fn create_checkout(
        &self,
        price_id: &PriceId,
        region: &str,
        redirect_uri: Option<&str>,
    ) -> CloudResult<CheckoutResponse> {
        let request = CheckoutRequest {
            price_id: price_id.clone(),
            region: region.to_owned(),
            redirect_uri: redirect_uri.map(String::from),
        };
        self.post(ApiPaths::CLOUD_CHECKOUT, &request).await
    }

    pub async fn report_activity(&self, event_type: &str, user_id: &UserId) -> CloudResult<()> {
        let request = ActivityRequest {
            event: event_type.to_owned(),
            timestamp: Utc::now().to_rfc3339(),
            data: ActivityData {
                user_id: user_id.clone(),
            },
        };
        self.post_no_response(ApiPaths::CLOUD_ACTIVITY, &request)
            .await
    }
}
