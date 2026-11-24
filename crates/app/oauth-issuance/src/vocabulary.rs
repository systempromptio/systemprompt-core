//! The grant and token vocabulary the token endpoint serves, as the strings
//! an authorization-server metadata document (RFC 8414, RFC 8693) advertises.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_oauth::services::validation::id_jag::{ID_JAG_GRANT_PROFILE, ID_JAG_TOKEN_TYPE};
use systemprompt_oauth::{GrantType, PkceMethod, ResponseType, TokenAuthMethod};

use crate::token_exchange::{ACCESS_TOKEN_TYPE, ID_TOKEN_TYPE};

/// The response types, grants, PKCE methods, client authentication methods,
/// token-exchange token types and grant profiles the token endpoint serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationServerVocabulary {
    pub response_types: Vec<String>,
    pub grant_types: Vec<String>,
    pub code_challenge_methods: Vec<String>,
    pub token_endpoint_auth_methods: Vec<String>,
    pub subject_token_types: Vec<String>,
    pub issued_token_types: Vec<String>,
    pub grant_profiles: Vec<String>,
}

impl AuthorizationServerVocabulary {
    #[must_use]
    pub fn served() -> Self {
        Self {
            response_types: vec![ResponseType::Code.to_string()],
            grant_types: vec![
                GrantType::AuthorizationCode.to_string(),
                GrantType::RefreshToken.to_string(),
                GrantType::TokenExchange.to_string(),
                GrantType::JwtBearer.to_string(),
            ],
            code_challenge_methods: vec![PkceMethod::S256.to_string()],
            token_endpoint_auth_methods: vec![
                TokenAuthMethod::None.to_string(),
                TokenAuthMethod::ClientSecretPost.to_string(),
                TokenAuthMethod::ClientSecretBasic.to_string(),
            ],
            subject_token_types: vec![
                ACCESS_TOKEN_TYPE.to_owned(),
                ID_TOKEN_TYPE.to_owned(),
                ID_JAG_TOKEN_TYPE.to_owned(),
            ],
            issued_token_types: vec![ACCESS_TOKEN_TYPE.to_owned(), ID_JAG_TOKEN_TYPE.to_owned()],
            grant_profiles: vec![ID_JAG_GRANT_PROFILE.to_owned()],
        }
    }
}
