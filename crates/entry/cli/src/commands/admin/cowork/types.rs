use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DeviceCertId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceCertEnrolledOutput {
    pub id: DeviceCertId,
    pub user_id: UserId,
    pub fingerprint: String,
    pub label: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeCodeIssuedOutput {
    pub user_id: UserId,
    pub code: String,
    pub expires_at: DateTime<Utc>,
    pub message: String,
}
