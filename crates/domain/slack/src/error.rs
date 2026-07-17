//! Slack integration error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum SlackError {
        common: [json, validation, config, http],

        #[error("signature verification failed: {0}")]
        Signature(String),

        #[error("request timestamp outside tolerance window")]
        StaleTimestamp,

        #[error("unknown Slack app for workspace {0}")]
        UnknownWorkspace(String),

        #[error("malformed Slack request: {0}")]
        MalformedRequest(String),

        #[error("no agent routed for {0}")]
        NoAgentRouted(String),

        #[error("outbound Slack API error: {0}")]
        Outbound(String),

        #[error("invalid outbound URL: {0}")]
        OutboundUrl(String),

        #[error("{0}")]
        Internal(String),
    }
}

pub type SlackResult<T> = Result<T, SlackError>;
