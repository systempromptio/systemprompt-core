//! Teams integration error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;
use systemprompt_models::net::OutboundUrlError;

domain_error! {
    pub enum TeamsError {
        common: [json, validation, config, http],

        #[error("activity token validation failed: token missing kid")]
        MissingKeyId,

        #[error("activity token validation failed: unknown signing key '{kid}'")]
        UnknownSigningKey { kid: String },

        #[error(
            "activity token validation failed: serviceurl claim '{claim}' does not match activity serviceUrl '{activity}'"
        )]
        ServiceUrlMismatch { claim: String, activity: String },

        #[error("activity token validation failed: token missing serviceurl claim")]
        MissingServiceUrl,

        #[error("activity token validation failed: {context}")]
        InvalidToken {
            context: &'static str,
            #[source]
            source: jsonwebtoken::errors::Error,
        },

        #[error("token issuer mismatch: {0}")]
        IssuerMismatch(&'static str),

        #[error("token audience mismatch: {0}")]
        AudienceMismatch(systemprompt_identifiers::TeamsAppId),

        #[error("activity token outside tolerance window")]
        StaleToken,

        #[error("malformed Teams activity: unhandled type '{kind}'")]
        UnhandledActivityType { kind: String },

        #[error("malformed Teams activity: missing tenant id")]
        MissingTenantId,

        #[error("malformed Teams activity: invalid sender id")]
        InvalidSenderId(#[source] systemprompt_identifiers::error::IdValidationError),

        #[error("outbound Bot Connector error: {status}: {body}")]
        Outbound {
            status: reqwest::StatusCode,
            body: String,
        },

        #[error("Bot Framework token endpoint returned {status}: {body}")]
        TokenEndpoint {
            status: reqwest::StatusCode,
            body: String,
        },

        #[error("invalid outbound URL: {0}")]
        OutboundUrl(#[from] OutboundUrlError),

        #[error("guarded outbound http client is unavailable")]
        ClientUnavailable,
    }
}

pub type TeamsResult<T> = Result<T, TeamsError>;
