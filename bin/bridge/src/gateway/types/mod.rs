//! Wire types exchanged with the systemprompt gateway.
//!
//! Provisioned OAuth client credentials and plugin hook tokens live here; the
//! identity, enrolment and release-feed bodies are shared with the gateway
//! through [`systemprompt_models::bridge::gateway`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub use auth::*;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ClientId;
pub use systemprompt_models::bridge::gateway::{
    ReleaseManifest, SelfEnrollRequest, SelfEnrollResponse, WhoamiResponse,
};

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
