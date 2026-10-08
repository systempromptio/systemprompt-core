//! Slack integration error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;
use systemprompt_models::net::OutboundUrlError;

domain_error! {
    pub enum SlackError {
        common: [json, validation, config, http],

        #[error("signature verification failed: signing secret is empty; refusing to verify")]
        EmptySigningSecret,

        #[error("signature verification failed: missing v0= prefix")]
        MissingSignaturePrefix,

        #[error("signature verification failed: signature is not valid hex")]
        SignatureEncoding(#[source] hex::FromHexError),

        #[error("signature verification failed: HMAC mismatch")]
        SignatureMismatch(#[source] hmac::digest::MacError),

        #[error("signing secret rejected by HMAC")]
        SigningKey(#[source] hmac::digest::InvalidLength),

        #[error("malformed Slack request: invalid X-Slack-Request-Timestamp")]
        InvalidTimestamp(#[source] std::num::ParseIntError),

        #[error("request timestamp outside tolerance window")]
        StaleTimestamp,

        #[error("outbound Slack API error: {0}")]
        Outbound(String),

        #[error("invalid outbound URL: {0}")]
        OutboundUrl(#[from] OutboundUrlError),
    }
}

pub type SlackResult<T> = Result<T, SlackError>;
