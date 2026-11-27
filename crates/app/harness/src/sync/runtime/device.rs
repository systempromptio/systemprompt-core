//! The device this install enrolled for the account's user: enrolled on first
//! need with the install's fingerprint and the device label, kept sealed with
//! the account, handed to governance as device evidence. A refused enrolment is
//! tried again after 2 s, doubling up to an hour; a refused device credential
//! forgets the device, so the next capture enrols again. A device enrolled for
//! another user is not used.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_accounts::DeviceEntry;
use sp_client::with_jwt;
use sp_protocol::DeviceCredential;
use sp_protocol::ids::{ConsumerInstallationId, DeviceId, UserId};
use sp_protocol::names::device_fingerprint;
use sp_protocol::wire::consumer::DeviceEnrolRequest;
use uuid::Uuid;

use super::{RuntimeFiles, WaitKey};
use crate::cadence::enrol_delay;
use crate::poll::Scope;
use crate::ports::{Instance, Store};
use crate::target::Target;

#[derive(Debug, Clone)]
pub(super) struct HeldDevice {
    pub(super) id: DeviceId,
    pub(super) installation: ConsumerInstallationId,
    pub(super) credential: DeviceCredential,
}

impl RuntimeFiles {
    pub(super) async fn device<S: Store, T: Target>(
        &mut self,
        scope: &Scope<S, T>,
        user: &UserId,
        enrol: bool,
    ) -> Option<HeldDevice> {
        let stored = match scope.store.device(&scope.account) {
            Ok(stored) => stored,
            Err(error) => {
                tracing::warn!(account = %scope.account, error = %error, "the enrolled device could not be read");
                return None;
            },
        };
        let held = match stored.filter(|entry| &entry.user == user) {
            Some(entry) => Some(complete(scope, entry)),
            None if enrol => self.enrol(scope, user).await,
            None => None,
        }?;
        scope.target.set_device(Some(held.credential.clone()));
        Some(held)
    }

    async fn enrol<S: Store, T: Target>(
        &mut self,
        scope: &Scope<S, T>,
        user: &UserId,
    ) -> Option<HeldDevice> {
        let now = scope.clock.now();
        if !self.due(&WaitKey::Enrol, now) {
            return None;
        }
        let request = DeviceEnrolRequest {
            fingerprint: device_fingerprint(&scope.store.install_id(), user),
            label: self.device_label.clone(),
        };
        let source = scope.store.credentials(&scope.account);
        let instance = &scope.instance;
        let body = &request;
        let enrolled = with_jwt(&*source, move |jwt| async move {
            instance.enrol_device(&jwt, body).await
        })
        .await;
        let enrolment = match enrolled {
            Ok(enrolment) if &enrolment.consumer_id == user => enrolment,
            Ok(_) => {
                tracing::warn!(account = %scope.account, "the instance enrolled the device for another user");
                self.failed(WaitKey::Enrol, now, enrol_delay);
                return None;
            },
            Err(error) => {
                tracing::warn!(account = %scope.account, error = %error, "device enrolment failed");
                self.failed(WaitKey::Enrol, now, enrol_delay);
                return None;
            },
        };
        self.waits.remove(&WaitKey::Enrol);
        let entry = DeviceEntry {
            device_id: enrolment.device_id,
            user: enrolment.consumer_id,
            installation: Some(Uuid::new_v4().hyphenated().to_string()),
            credential: enrolment.credential,
        };
        if let Err(error) = scope.store.set_device(&scope.account, &entry) {
            tracing::warn!(account = %scope.account, error = %error, "keeping the device credential failed");
        }
        tracing::info!(account = %scope.account, device = %entry.device_id, "device enrolled");
        Some(complete(scope, entry))
    }
}

pub(super) fn held_device<S: Store, T: Target>(scope: &Scope<S, T>) -> Option<HeldDevice> {
    let entry = scope.store.device(&scope.account).ok().flatten()?;
    let installation = ConsumerInstallationId::new(entry.installation?);
    Some(HeldDevice {
        id: entry.device_id,
        installation,
        credential: entry.credential,
    })
}

pub(super) fn forget_device<S: Store, T: Target>(scope: &Scope<S, T>) {
    tracing::warn!(
        account = %scope.account,
        "the instance refused the device credential; forgetting the device"
    );
    scope.target.set_device(None);
    if let Err(error) = scope.store.forget_device(&scope.account) {
        tracing::warn!(account = %scope.account, error = %error, "forgetting the device failed");
    }
}

fn complete<S: Store, T: Target>(scope: &Scope<S, T>, mut entry: DeviceEntry) -> HeldDevice {
    let installation = if let Some(installation) = entry.installation.clone() {
        installation
    } else {
        let minted = Uuid::new_v4().hyphenated().to_string();
        entry.installation = Some(minted.clone());
        if let Err(error) = scope.store.set_device(&scope.account, &entry) {
            tracing::warn!(
                account = %scope.account,
                error = %error,
                "recording the installation id failed"
            );
        }
        minted
    };
    HeldDevice {
        id: entry.device_id,
        installation: ConsumerInstallationId::new(installation),
        credential: entry.credential,
    }
}
