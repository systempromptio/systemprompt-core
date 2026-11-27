//! The push token seam: what the delivery pipeline reads of a user's devices.
//!
//! The member that owns device records provides a [`DynPushTokenDirectory`];
//! the push channels read it. A channel asks for every live token of a
//! recipient's unrevoked devices and drops the token of a device a provider
//! reports as unregistered.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{DeviceId, UserId};
use systemprompt_models::devices::AppIdentifier;
use systemprompt_models::notifications::{PushProvider, PushToken};
use systemprompt_traits::{BoxedSource, RepositoryError};

/// A device's push token as the delivery pipeline needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePushToken {
    pub device_id: DeviceId,
    pub provider: PushProvider,
    pub token: PushToken,
    pub app_id: AppIdentifier,
}

/// Why the push token directory could not answer.
#[derive(Debug, thiserror::Error)]
pub enum PushTokenDirectoryError {
    #[error("push token query failed")]
    Repository(#[from] RepositoryError),
    #[error("push token directory failed")]
    Failed(#[source] BoxedSource),
}

/// The push tokens of users' devices.
///
/// `tokens_for` answers every live token of the user's unrevoked devices.
/// `revoke` drops exactly the token it is handed: a device that registered a
/// new token since the caller read this one keeps the new one. An absent
/// token is not an error. Held as `Arc<dyn PushTokenDirectory>`, so it uses
/// `#[async_trait]`: native `async fn` in traits is not `dyn`-compatible.
#[async_trait::async_trait]
pub trait PushTokenDirectory: Send + Sync {
    async fn tokens_for(
        &self,
        user: &UserId,
    ) -> Result<Vec<DevicePushToken>, PushTokenDirectoryError>;

    async fn revoke(&self, token: &DevicePushToken) -> Result<(), PushTokenDirectoryError>;
}

pub type DynPushTokenDirectory = Arc<dyn PushTokenDirectory>;
