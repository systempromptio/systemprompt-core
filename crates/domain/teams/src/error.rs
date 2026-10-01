//! Teams integration error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;
use systemprompt_models::net::OutboundUrlError;

domain_error! {
    pub enum TeamsError {
        common: [json, validation, config, http],

        #[error("activity token validation failed: {0}")]
        TokenValidation(String),

        #[error("activity token validation failed: {context}")]
        InvalidToken {
            context: &'static str,
            #[source]
            source: jsonwebtoken::errors::Error,
        },

        #[error("token issuer mismatch: {0}")]
        IssuerMismatch(String),

        #[error("token audience mismatch: {0}")]
        AudienceMismatch(String),

        #[error("activity token outside tolerance window")]
        StaleToken,

        #[error("unknown Teams app for tenant {0}")]
        UnknownTenant(String),

        #[error("malformed Teams activity: {0}")]
        MalformedActivity(String),

        #[error("no agent routed for {0}")]
        NoAgentRouted(String),

        #[error("outbound Bot Connector error: {0}")]
        Outbound(String),

        #[error("invalid outbound URL: {0}")]
        OutboundUrl(#[from] OutboundUrlError),

        #[error("guarded outbound http client is unavailable")]
        ClientUnavailable,

        #[error("{0}")]
        Internal(String),
    }
}

pub type TeamsResult<T> = Result<T, TeamsError>;
