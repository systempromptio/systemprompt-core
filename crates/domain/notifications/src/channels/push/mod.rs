//! What the two push channels share: the payload, the sender seam, the
//! delivery loop, foreground presence and the provider call seam.
//!
//! A push carries exactly a [`PushPayload`]: the notification's opaque id, a
//! category and a collapse key. It never carries the title, the body or
//! anything else of the notification, because a provider stores what it is
//! handed; the app shows its own fixed line for the category and fetches the
//! notification from the instance.
//!
//! The category is [`PushCategory::Approval`] for a notification whose kind
//! has an `approval_requested` segment and [`PushCategory::Notification`]
//! otherwise. The collapse key is the notification id, so a redelivered or
//! coalesced notification replaces its earlier push on the device.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assemble;
mod channel;
mod error;
mod http;
mod presence;

use systemprompt_extension::notifications::DevicePushToken;
use systemprompt_models::notifications::{
    CollapseKey, Notification, NotificationKind, PushCategory, PushPayload, PushProvider,
};

pub use assemble::{PushParts, push_channels};
pub use channel::PushChannel;
pub use error::PushError;
pub use http::{
    ProviderBearer, ProviderCredential, ProviderHttp, ProviderRequest, ProviderResponse,
    ReqwestProviderHttp,
};
pub use presence::{ForegroundPresence, StreamPresence};

const APPROVAL_SEGMENT: &str = "approval_requested";

/// What a provider said of one push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushVerdict {
    Accepted,
    Unregistered,
}

/// Hands one payload to one provider for one device token.
///
/// `serves` says whether a token is this sender's to push to; the default is
/// every token its provider issued. A channel is generic over its sender, so
/// nothing holds one behind `dyn`.
pub trait PushSender: Send + Sync + 'static {
    fn provider(&self) -> PushProvider;

    fn serves(&self, token: &DevicePushToken) -> bool {
        token.provider == self.provider()
    }

    fn send(
        &self,
        token: &DevicePushToken,
        payload: &PushPayload,
    ) -> impl Future<Output = Result<PushVerdict, PushError>> + Send;
}

#[must_use]
pub fn category_of(kind: &NotificationKind) -> PushCategory {
    if kind
        .as_str()
        .split('.')
        .any(|segment| segment == APPROVAL_SEGMENT)
    {
        PushCategory::Approval
    } else {
        PushCategory::Notification
    }
}

pub fn payload_for(notification: &Notification) -> Result<PushPayload, PushError> {
    Ok(PushPayload::new(
        notification.id.clone(),
        category_of(&notification.kind),
        CollapseKey::try_new(notification.id.as_str())?,
    ))
}
