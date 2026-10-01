//! Serializable output types for the `admin session` command tree.
//!
//! These DTOs back the rendered show, list, logout, and switch results,
//! describing cached sessions, profile routing, and per-command outcomes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, ProfileName, SessionId, TenantId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    pub key: String,
    #[schemars(with = "Option<String>")]
    pub profile_name: Option<ProfileName>,
    pub user_email: String,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub is_active: bool,
    pub is_expired: bool,
    pub expires_in: Option<String>,
    pub stale_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutingInfo {
    #[schemars(with = "Option<String>")]
    pub profile_name: Option<ProfileName>,
    pub target: String,
    #[serde(rename = "tenant_id")]
    pub tenant: Option<TenantId>,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionShowOutput {
    pub sessions: Vec<SessionInfo>,
    pub routing: Option<RoutingInfo>,
}

pub use systemprompt_models::profile::ProfileInfo;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileListOutput {
    pub profiles: Vec<ProfileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogoutOutput {
    pub action: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SwitchOutput {
    #[schemars(with = "Option<String>")]
    pub previous_profile: Option<ProfileName>,
    #[schemars(with = "String")]
    pub new_profile: ProfileName,
    pub session_key: String,
    #[serde(rename = "tenant_id")]
    pub tenant: Option<TenantId>,
    pub message: String,
}
