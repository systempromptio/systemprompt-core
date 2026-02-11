use crate::auth::JwtAudience;
use serde::{Deserialize, Serialize};

const fn default_allow_registration() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(rename = "jwt_issuer")]
    pub issuer: String,

    /// JWT access token expiration in seconds.
    /// Maximum: 31,536,000 seconds (1 year / 8760 hours)
    /// Default: 2,592,000 seconds (30 days)
    #[serde(rename = "jwt_access_token_expiration")]
    pub access_token_expiration: i64,

    /// JWT refresh token expiration in seconds.
    /// Default: 15,552,000 seconds (180 days)
    #[serde(rename = "jwt_refresh_token_expiration")]
    pub refresh_token_expiration: i64,

    #[serde(rename = "jwt_audiences")]
    pub audiences: Vec<JwtAudience>,

    #[serde(default = "default_allow_registration")]
    pub allow_registration: bool,
}
