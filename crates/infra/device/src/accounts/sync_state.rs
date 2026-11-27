//! What the sync keeps per account in the store: the enrolled device with its
//! sealed credential, the outbox pointer, the last applied manifest version and
//! the user id.
//!
//! The device credential is opened for one call. A credential this computer
//! will never open again, or one that does not parse, is forgotten so the
//! caller enrols again; one the OS cannot open right now is kept and reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_protocol::ids::{AccountId, DeviceId, UserId};
use sp_protocol::{DeviceCredential, ManifestVersion};
use crate::secure::SealedString;

use crate::accounts::accounts::Accounts;
use crate::accounts::error::AccountsError;
use crate::accounts::store::DeviceRecord;

/// The device this install enrolled for an account's user, with its credential
/// opened for one call.
#[derive(Debug, Clone)]
pub struct DeviceEntry {
    pub device_id: DeviceId,
    pub user: UserId,
    pub installation: Option<String>,
    pub credential: DeviceCredential,
}

impl Accounts {
    pub fn device(&self, id: &AccountId) -> Result<Option<DeviceEntry>, AccountsError> {
        let Some(held) = self.read(id, |record, _| record.device.clone())? else {
            return Ok(None);
        };
        let opened = match held.credential.open(&*self.inner.sealer) {
            Ok(opened) => opened,
            Err(error) if error.keeps_ciphertext() => {
                return Err(AccountsError::SecureStorageUnavailable(error));
            },
            Err(error) => {
                tracing::warn!(
                    account = %id,
                    error = %error,
                    "the stored device credential will not open; forgetting it"
                );
                self.forget_device(id)?;
                return Ok(None);
            },
        };
        match DeviceCredential::new(opened.as_str()) {
            Ok(credential) => Ok(Some(DeviceEntry {
                device_id: held.device_id,
                user: held.user,
                installation: held.installation,
                credential,
            })),
            Err(error) => {
                tracing::warn!(
                    account = %id,
                    error = %error,
                    "the stored device credential is malformed; forgetting it"
                );
                self.forget_device(id)?;
                Ok(None)
            },
        }
    }

    pub fn set_device(&self, id: &AccountId, device: &DeviceEntry) -> Result<(), AccountsError> {
        let sealed = SealedString::seal(&*self.inner.sealer, device.credential.expose())
            .map_err(AccountsError::Seal)?;
        self.mutate_store(|store, volatile| {
            let record = store.find_mut(id)?;
            if record.pat.is_none() && !volatile.contains(id) {
                return Err(AccountsError::NotSignedIn {
                    account: id.clone(),
                });
            }
            record.device = Some(DeviceRecord {
                device_id: device.device_id.clone(),
                user: device.user.clone(),
                installation: device.installation.clone(),
                credential: sealed,
                extra: serde_json::Map::new(),
            });
            Ok(())
        })
    }

    pub fn forget_device(&self, id: &AccountId) -> Result<(), AccountsError> {
        self.mutate_store(|store, _| {
            store.find_mut(id)?.device = None;
            Ok(())
        })
    }

    pub fn outbox(&self, id: &AccountId) -> Result<Option<String>, AccountsError> {
        self.read(id, |record, _| record.outbox.clone())
    }

    pub fn set_outbox(&self, id: &AccountId, pointer: Option<String>) -> Result<(), AccountsError> {
        self.mutate_store(|store, _| {
            store.find_mut(id)?.outbox = pointer;
            Ok(())
        })
    }

    pub fn record_applied_version(
        &self,
        id: &AccountId,
        version: &ManifestVersion,
    ) -> Result<(), AccountsError> {
        self.mutate_store(|store, _| {
            store.find_mut(id)?.applied_version = Some(version.clone());
            Ok(())
        })
    }

    pub fn last_applied_version(
        &self,
        id: &AccountId,
    ) -> Result<Option<ManifestVersion>, AccountsError> {
        self.read(id, |record, _| record.applied_version.clone())
    }

    pub fn user_id(&self, id: &AccountId) -> Result<Option<UserId>, AccountsError> {
        self.read(id, |record, _| record.user_id.clone())
    }
}
