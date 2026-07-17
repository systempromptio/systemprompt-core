//! Wire types exchanged with the systemprompt gateway: provisioned OAuth client
//! credentials, plugin hook tokens, and the `whoami` identity envelope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::{ClientId, TenantId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeOAuthClientResponse {
    pub client_id: ClientId,
    pub client_secret: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub token_endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HookTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    pub expires_in: i64,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiResponse {
    #[serde(default)]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
