//! Why a `login` step failed: reading what the user supplied, resolving the
//! gateway, redeeming a code, or enrolling the device afterwards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;

/// What is wrong with the text pasted back from the device-link page.
#[derive(Debug, thiserror::Error)]
pub enum PastedCodeError {
    #[error("nothing pasted")]
    Empty,
    #[error(
        "that looks like a command but carries no `--code` — paste just the code, or the whole \
         command the page displayed"
    )]
    CommandWithoutCode,
    #[error("the sign-in was not approved ({reason})")]
    NotApproved { reason: String },
    #[error("that URL carries no `code` parameter — paste the code the page displayed")]
    UrlWithoutCode,
}

/// A failed `login` step, with the cause it came from.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("could not read {what} from stdin: {source}")]
    Stdin {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("stdin carried no PAT")]
    EmptyStdin,
    #[error(
        "signing in interactively needs a terminal. Unattended, redeem an administrator-issued \
         code with `{bin} login --code <exchange-code>`, or pipe a PAT into `{bin} login --stdin`"
    )]
    NotATerminal { bin: &'static str },
    #[error("could not bind the loopback callback listener: {0}")]
    Loopback(#[source] crate::auth::loopback::LoopbackError),
    #[error("{0}")]
    SignIn(#[source] Box<crate::auth::providers::AuthError>),
    #[error(transparent)]
    PastedCode(#[from] PastedCodeError),
    #[error("{0}")]
    Config(#[source] crate::config::ConfigReadError),
    #[error("--gateway: {0}")]
    GatewayFlag(#[source] IdValidationError),
    #[error("{0}")]
    Gateway(#[source] Box<crate::gateway::GatewayError>),
    #[error("the install identity could not be read, so this device is not enrolled")]
    InstallIdUnreadable,
    #[error("{0}")]
    Credential(#[source] Box<crate::auth::ChainError>),
    #[error("whoami carried no user id")]
    NoUserId,
    #[error("{0}")]
    Enrolment(#[source] Box<crate::feedback::FeedbackError>),
}
