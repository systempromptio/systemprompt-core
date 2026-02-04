use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    pub key: String,
    pub profile_name: String,
    pub user_email: String,
    pub session_id: String,
    pub context_id: String,
    pub is_active: bool,
    pub is_expired: bool,
    pub expires_in: Option<String>,
    pub stale_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutingInfo {
    pub profile_name: String,
    pub target: String,
    pub tenant_id: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionShowOutput {
    pub sessions: Vec<SessionInfo>,
    pub routing: Option<RoutingInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileInfo {
    pub name: String,
    pub routing: String,
    pub is_active: bool,
    pub session_status: String,
}

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
    pub tenant_id: Option<String>,
    pub message: String,
}
