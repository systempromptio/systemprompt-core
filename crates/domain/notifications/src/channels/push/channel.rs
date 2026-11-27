//! [`PushChannel`]: the delivery loop both push providers share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::notifications::{
    ChannelError, DynPushTokenDirectory, NotificationChannel,
};
use systemprompt_models::notifications::{Attention, Notification};

use super::{ForegroundPresence, PushError, PushSender, PushVerdict, payload_for};

/// Delivers a notification to every device token one provider serves.
///
/// Only a notification at human attention is pushed. A recipient with a live
/// event stream is reached by the in-app channel and is not pushed to. A
/// token the provider reports as unregistered is revoked in the same
/// delivery. When any device fails the delivery fails, with a failure worth
/// retrying reported ahead of one that is not; a repeated push replaces the
/// earlier one on the device through the collapse key.
pub struct PushChannel<S> {
    sender: S,
    directory: DynPushTokenDirectory,
    presence: Arc<dyn ForegroundPresence>,
}

impl<S: PushSender> PushChannel<S> {
    #[must_use]
    pub fn new(
        sender: S,
        directory: DynPushTokenDirectory,
        presence: Arc<dyn ForegroundPresence>,
    ) -> Self {
        Self {
            sender,
            directory,
            presence,
        }
    }

    async fn push(&self, notification: &Notification) -> Result<(), PushError> {
        if self.presence.is_foreground(&notification.recipient).await {
            return Ok(());
        }
        let tokens = self.directory.tokens_for(&notification.recipient).await?;
        let payload = payload_for(notification)?;
        let provider = self.sender.provider();
        let mut failure: Option<PushError> = None;
        for token in tokens.iter().filter(|token| self.sender.serves(token)) {
            let outcome = match self.sender.send(token, &payload).await {
                Ok(PushVerdict::Accepted) => Ok(()),
                Ok(PushVerdict::Unregistered) => self
                    .directory
                    .revoke(&token.device_id, provider)
                    .await
                    .map_err(PushError::from),
                Err(error) => Err(error),
            };
            if let Err(error) = outcome {
                tracing::warn!(
                    device_id = %token.device_id,
                    surface = provider.as_str(),
                    error = %error,
                    "Push to a device failed"
                );
                failure = Some(match failure {
                    Some(held) if held.is_retryable() || !error.is_retryable() => held,
                    _ => error,
                });
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

impl<S: PushSender> fmt::Debug for PushChannel<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PushChannel")
            .field("provider", &self.sender.provider())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<S: PushSender> NotificationChannel for PushChannel<S> {
    fn surface(&self) -> &'static str {
        self.sender.provider().as_str()
    }

    fn delivers(&self, attention: Attention) -> bool {
        attention == Attention::Human
    }

    async fn deliver(&self, notification: &Notification, _unread: i64) -> Result<(), ChannelError> {
        self.push(notification)
            .await
            .map_err(|error| error.into_channel_error(self.surface()))
    }
}
