//! The profile tab payload: identity, plan, and usage as the webview receives
//! it on `profile.fetch`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;
use systemprompt_identifiers::{TenantId, UserId};
use systemprompt_models::api::cloud::BridgeProfileUsage;

use crate::gateway::types::BridgeProfile;

#[derive(Debug, Clone, Serialize)]
pub struct ProfileIdentity {
    pub email: Option<String>,
    pub user_id: Option<UserId>,
    pub tenant_id: Option<TenantId>,
    pub display_name: Option<String>,
    pub provider: Option<String>,
    pub roles: Vec<String>,
    pub exp_unix: Option<u64>,
    pub verified_at_unix: Option<u64>,
    pub token_length: Option<usize>,
    pub token_ttl_seconds: Option<u64>,
    // JSON: white-label identity envelope passthrough from the gateway whoami
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub gateway: String,
    pub identity: ProfileIdentity,
    pub bridge_profile: BridgeProfile,
    pub usage: BridgeProfileUsage,
}
