use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, SessionId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    pub key: String,
    pub profile_name: String,
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
    pub profile_name: String,
    pub target: String,
    #[serde(rename = "tenant_id")]
    pub tenant: Option<String>,
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
    pub previous_profile: Option<String>,
    pub new_profile: String,
    pub session_key: String,
    #[serde(rename = "tenant_id")]
    pub tenant: Option<String>,
    pub message: String,
}
