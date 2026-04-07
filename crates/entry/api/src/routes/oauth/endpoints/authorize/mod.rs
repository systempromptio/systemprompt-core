mod handler;
pub mod response_builder;
pub(crate) mod validation;

pub use handler::{handle_authorize_get, handle_authorize_post};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ClientId;

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub response_type: String,
    pub client_id: ClientId,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub response_mode: Option<String>,
    pub display: Option<String>,
    pub prompt: Option<String>,
    pub max_age: Option<i64>,
    pub ui_locales: Option<String>,
    pub resource: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: ClientId,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub user_consent: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub resource: Option<String>,
}
