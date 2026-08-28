//! Cloud Management API types shared between CLI and API server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod tenant;
mod usage;

pub use tenant::{
    CloudEnterpriseLicenseInfo, CloudPlan, CloudPlanInfo, CloudTenant, CloudTenantInfo,
    CloudTenantSecrets, CloudTenantStatus, CloudTenantStatusResponse, RotateCredentialsResponse,
    SubscriptionStatus,
};
pub use usage::{
    BridgeProfileUsage, ConversationGroup, ConversationSummary, ModelShare,
    RecentConversationSummary, UsageWindow,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudApiError {
    pub error: CloudApiErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudApiErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudUserInfo {
    pub id: systemprompt_identifiers::UserId,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCustomerInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMeResponse {
    pub user: CloudUserInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<CloudCustomerInfo>,
    #[serde(default)]
    pub tenants: Vec<CloudTenantInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enterprise: Option<CloudEnterpriseLicenseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudListResponse<T> {
    pub data: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryToken {
    pub registry: String,
    pub username: String,
    pub token: String,
    pub repository: String,
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudStatusResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSecretsRequest {
    pub secrets: HashMap<String, String>,
}

pub type ApiResponse<T> = CloudApiResponse<T>;
pub type ApiError = CloudApiError;
pub type ApiErrorDetail = CloudApiErrorDetail;
pub type UserInfo = CloudUserInfo;
pub type CustomerInfo = CloudCustomerInfo;
pub type PlanInfo = CloudPlanInfo;
pub type Plan = CloudPlan;
pub type TenantInfo = CloudTenantInfo;
pub type Tenant = CloudTenant;
pub type TenantStatus = CloudTenantStatusResponse;
pub type TenantSecrets = CloudTenantSecrets;
pub type ListResponse<T> = CloudListResponse<T>;
pub type StatusResponse = CloudStatusResponse;
pub type EnterpriseLicenseInfo = CloudEnterpriseLicenseInfo;
