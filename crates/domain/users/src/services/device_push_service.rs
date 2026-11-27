//! A device's push token and what the device reports about itself.
//!
//! A caller writes only its own device's token. A request names no device
//! id: the caller presents the fingerprint its device was enrolled under,
//! and the device is the active record that holds that fingerprint and
//! belongs to the caller. An unknown fingerprint, a revoked device and
//! another user's device are refused with one answer,
//! [`DevicePushError::DeviceNotEnrolled`], so the refusal says nothing about
//! which it was.
//!
//! [`DevicePushService`] is also the users member's
//! [`PushTokenDirectory`]: the live tokens of a user's unrevoked devices, and
//! the removal of one a provider reports as unregistered.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::notifications::{
    DevicePushToken, PushTokenDirectory, PushTokenDirectoryError,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::devices::{AdminDeviceRow, DeviceClientInfo, DevicePlatform};
use systemprompt_models::notifications::{
    PushProvider, PushTokenRegistration, PushTokenRevocation, RegisterPushTokenRequest,
};

use super::device_cert_service::normalize_fingerprint;
use crate::error::UserError;
use crate::models::UserDeviceCert;
use crate::repository::{UpsertDevicePushTokenParams, UserRepository, device_id_of};

pub const MAX_OS_VERSION_LEN: usize = 64;
pub const ADMIN_DEVICE_LIST_LIMIT: i64 = 500;

/// Why a device's push token was not written, removed or listed.
#[derive(Debug, thiserror::Error)]
pub enum DevicePushError {
    #[error("device fingerprint is not a SHA-256 hex digest")]
    InvalidFingerprint(#[source] UserError),
    #[error("no active device is enrolled to the caller under this fingerprint")]
    DeviceNotEnrolled,
    #[error("push provider {provider} does not serve platform {platform}")]
    ProviderMismatch {
        provider: PushProvider,
        platform: DevicePlatform,
    },
    #[error(
        "operating-system version must be 1 to {MAX_OS_VERSION_LEN} visible ASCII characters or \
         spaces"
    )]
    InvalidOsVersion,
    #[error(transparent)]
    Users(#[from] UserError),
}

#[derive(Debug, Clone)]
pub struct DevicePushService {
    repository: Arc<UserRepository>,
}

impl DevicePushService {
    pub const fn new(repository: Arc<UserRepository>) -> Self {
        Self { repository }
    }

    pub async fn register(
        &self,
        caller: &UserId,
        fingerprint: &str,
        request: &RegisterPushTokenRequest,
    ) -> Result<PushTokenRegistration, DevicePushError> {
        validate_client(request.provider, &request.client)?;
        let device = self.own_device(caller, fingerprint).await?;
        let registered_at = self
            .repository
            .upsert_device_push_token(UpsertDevicePushTokenParams {
                device_id: &device.id,
                user_id: caller,
                provider: request.provider,
                token: &request.token,
                client: &request.client,
            })
            .await?
            .ok_or(DevicePushError::DeviceNotEnrolled)?;
        Ok(PushTokenRegistration {
            device_id: device_id_of(&device.id)?,
            provider: request.provider,
            registered_at,
        })
    }

    pub async fn revoke(
        &self,
        caller: &UserId,
        fingerprint: &str,
    ) -> Result<PushTokenRevocation, DevicePushError> {
        let device = self.own_device(caller, fingerprint).await?;
        let revoked = self
            .repository
            .delete_device_push_tokens(&device.id, caller)
            .await?;
        Ok(PushTokenRevocation {
            device_id: device_id_of(&device.id)?,
            revoked,
        })
    }

    pub async fn list_devices(
        &self,
        user: Option<&UserId>,
    ) -> Result<Vec<AdminDeviceRow>, DevicePushError> {
        Ok(self
            .repository
            .list_admin_devices(user, ADMIN_DEVICE_LIST_LIMIT)
            .await?)
    }

    async fn own_device(
        &self,
        caller: &UserId,
        fingerprint: &str,
    ) -> Result<UserDeviceCert, DevicePushError> {
        let fingerprint =
            normalize_fingerprint(fingerprint).map_err(DevicePushError::InvalidFingerprint)?;
        self.repository
            .find_active_device_cert_by_fingerprint(&fingerprint)
            .await?
            .filter(|device| device.user_id == *caller)
            .ok_or(DevicePushError::DeviceNotEnrolled)
    }
}

fn validate_client(
    provider: PushProvider,
    client: &DeviceClientInfo,
) -> Result<(), DevicePushError> {
    if provider.platform() != client.platform {
        return Err(DevicePushError::ProviderMismatch {
            provider,
            platform: client.platform,
        });
    }
    let os_version = client.os_version.as_str();
    let well_formed = !os_version.trim().is_empty()
        && os_version.len() <= MAX_OS_VERSION_LEN
        && os_version
            .bytes()
            .all(|b| b == b' ' || b.is_ascii_graphic());
    if well_formed {
        Ok(())
    } else {
        Err(DevicePushError::InvalidOsVersion)
    }
}

fn directory_error(error: UserError) -> PushTokenDirectoryError {
    match error {
        UserError::Repository(repository) => PushTokenDirectoryError::Repository(repository),
        other => PushTokenDirectoryError::Failed(Box::new(other)),
    }
}

#[async_trait]
impl PushTokenDirectory for DevicePushService {
    async fn tokens_for(
        &self,
        user: &UserId,
    ) -> Result<Vec<DevicePushToken>, PushTokenDirectoryError> {
        self.repository
            .list_live_push_tokens_for_user(user)
            .await
            .map_err(directory_error)
    }

    async fn revoke(&self, token: &DevicePushToken) -> Result<(), PushTokenDirectoryError> {
        self.repository
            .delete_device_push_token_for_provider(&token.device_id, token.provider, &token.token)
            .await
            .map(drop)
            .map_err(directory_error)
    }
}
