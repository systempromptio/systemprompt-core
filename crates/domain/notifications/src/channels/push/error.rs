//! [`PushError`]: why one push could not be handed to its provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::notifications::{ChannelError, PushTokenDirectoryError};
use systemprompt_identifiers::SecretName;
use systemprompt_models::notifications::CollapseKeyError;
use systemprompt_traits::BoxedSource;

/// Why a push failed, classified by whether another attempt can succeed.
///
/// A transport failure, a provider that is throttling or failing, a provider
/// token that could not be minted and a token directory that did not answer
/// are worth another attempt. A request the provider refuses, a credential
/// that is absent or unusable and a payload that cannot be built are not.
#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("push credential secret {secret} is not set")]
    CredentialMissing { secret: SecretName },
    #[error("push credential secret {secret} is not usable")]
    CredentialInvalid {
        secret: SecretName,
        #[source]
        source: BoxedSource,
    },
    #[error("the push provider token could not be minted")]
    Authorization(#[source] BoxedSource),
    #[error("the push provider could not be reached")]
    Transport(#[source] BoxedSource),
    #[error("the push provider answered {status} {reason}")]
    Unavailable { status: u16, reason: String },
    #[error("the push provider refused the push with {status} {reason}")]
    Rejected { status: u16, reason: String },
    #[error("the push payload could not be encoded")]
    Encode(#[from] serde_json::Error),
    #[error("the notification id is not a collapse key")]
    CollapseKey(#[from] CollapseKeyError),
    #[error("the push token directory failed")]
    Directory(#[from] PushTokenDirectoryError),
}

impl PushError {
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Authorization(_)
                | Self::Transport(_)
                | Self::Unavailable { .. }
                | Self::Directory(_)
        )
    }

    pub(super) fn into_channel_error(self, surface: &'static str) -> ChannelError {
        match self {
            Self::Encode(source) => ChannelError::Encode(source),
            other if other.is_retryable() => ChannelError::Other {
                surface,
                source: Box::new(other),
            },
            other => ChannelError::Rejected {
                surface,
                source: Box::new(other),
            },
        }
    }
}
