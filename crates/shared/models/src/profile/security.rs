use crate::auth::JwtAudience;
use serde::{Deserialize, Serialize};

const fn default_allow_registration() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(rename = "jwt_issuer")]
    pub issuer: String,

    #[serde(rename = "jwt_access_token_expiration")]
    pub access_token_expiration: i64,

    #[serde(rename = "jwt_refresh_token_expiration")]
    pub refresh_token_expiration: i64,

    #[serde(rename = "jwt_audiences")]
    pub audiences: Vec<JwtAudience>,

    #[serde(default = "default_allow_registration")]
    pub allow_registration: bool,
}
