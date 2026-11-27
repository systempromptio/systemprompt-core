//! The skills' runtime files: this install enrols as the user's device, reads
//! each projected skill publication's installation plan with the device
//! credential, has the applier write the files beside the projected `SKILL.md`
//! and read them back, and queues a receipt in the account's outbox until the
//! instance acknowledges it. Sessions governance reports with the device
//! evidence are bound to the acknowledged receipts. Nothing here changes the
//! sync status: what fails waits for the next capture.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod deliver;
mod device;
mod install;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use crate::projection::SkillPublication;
use sp_protocol::ids::{NativeSessionId, PublicationId, UserId};
use sp_protocol::wire::consumer::InstallationPlan;

use crate::sync::outbox::{self, Outbox};
use crate::sync::poll::Scope;
use crate::sync::ports::Store;
use crate::sync::target::Target;

use self::deliver::deliver;
use self::device::{HeldDevice, forget_device, held_device};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum WaitKey {
    Enrol,
    Publication(PublicationId),
}

#[derive(Debug, Clone, Copy)]
struct Wait {
    attempts: u32,
    next: DateTime<Utc>,
}

/// What one capture left: publications still to install and whether the outbox
/// still holds something to deliver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureOutcome {
    pub remaining: usize,
    pub undelivered: bool,
}

impl CaptureOutcome {
    pub const fn complete(self) -> bool {
        self.remaining == 0 && !self.undelivered
    }
}

/// One account's runtime-file state: the waits of failed enrolments and
/// installations and the plans last installed.
#[derive(Debug)]
pub struct RuntimeFiles {
    state_dir: PathBuf,
    device_label: String,
    waits: HashMap<WaitKey, Wait>,
    plans: HashMap<PublicationId, InstallationPlan>,
}

impl RuntimeFiles {
    pub fn new(state_dir: &Path, device_label: &str) -> Self {
        Self {
            state_dir: state_dir.to_path_buf(),
            device_label: device_label.to_owned(),
            waits: HashMap::new(),
            plans: HashMap::new(),
        }
    }

    pub async fn capture<S: Store, T: Target>(
        &mut self,
        scope: &Scope<S, T>,
        user: &UserId,
        publications: &[SkillPublication],
    ) -> CaptureOutcome {
        let incomplete = CaptureOutcome {
            remaining: publications.len(),
            undelivered: true,
        };
        let Some(device) = self.device(scope, user, !publications.is_empty()).await else {
            return CaptureOutcome {
                remaining: publications.len(),
                undelivered: false,
            };
        };
        let mut outbox = self.load_outbox(scope, &device).await;
        if deliver(
            &scope.instance,
            &device.credential,
            &mut outbox,
            &*scope.clock,
        )
        .await
        {
            forget_device(scope);
            return incomplete;
        }
        let installed = self
            .install_all(scope, &device, &mut outbox, publications)
            .await;
        if installed.device_refused
            || deliver(
                &scope.instance,
                &device.credential,
                &mut outbox,
                &*scope.clock,
            )
            .await
        {
            forget_device(scope);
            return incomplete;
        }
        self.save_outbox(scope, &outbox).await;
        let outcome = CaptureOutcome {
            remaining: installed.remaining,
            undelivered: outbox.undelivered(),
        };
        tracing::debug!(
            account = %scope.account,
            installed = installed.written,
            remaining = outcome.remaining,
            undelivered = outcome.undelivered,
            "skill runtime files captured"
        );
        outcome
    }

    pub async fn bind<S: Store, T: Target>(
        &self,
        scope: &Scope<S, T>,
        session: &NativeSessionId,
    ) -> bool {
        let Some(device) = held_device(scope) else {
            return false;
        };
        let mut outbox = self.load_outbox(scope, &device).await;
        outbox.bind_session(session);
        if deliver(
            &scope.instance,
            &device.credential,
            &mut outbox,
            &*scope.clock,
        )
        .await
        {
            forget_device(scope);
            return true;
        }
        self.save_outbox(scope, &outbox).await;
        outbox.undelivered()
    }

    fn due(&self, key: &WaitKey, now: DateTime<Utc>) -> bool {
        self.waits.get(key).is_none_or(|wait| wait.next <= now)
    }

    fn failed(&mut self, key: WaitKey, now: DateTime<Utc>, delay: fn(u32) -> std::time::Duration) {
        let attempts = self
            .waits
            .get(&key)
            .map_or(0, |wait| wait.attempts)
            .saturating_add(1);
        self.waits.insert(
            key,
            Wait {
                attempts,
                next: crate::sync::cadence::after(now, delay(attempts)),
            },
        );
    }

    async fn load_outbox<S: Store, T: Target>(
        &self,
        scope: &Scope<S, T>,
        device: &HeldDevice,
    ) -> Outbox {
        match scope.store.outbox(&scope.account) {
            Ok(Some(_)) => {},
            Ok(None) => return Outbox::new(device.id.clone()),
            Err(error) => {
                tracing::warn!(account = %scope.account, error = %error, "reading the outbox pointer failed");
                return Outbox::new(device.id.clone());
            },
        }
        let path = outbox::outbox_path(&self.state_dir, &outbox::file_name(&scope.account));
        let id = device.id.clone();
        let fallback = id.clone();
        tokio::task::spawn_blocking(move || outbox::load(&path, &id))
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(error = %error, "reading the outbox did not finish");
                Outbox::new(fallback)
            })
    }

    async fn save_outbox<S: Store, T: Target>(&self, scope: &Scope<S, T>, held: &Outbox) {
        let name = outbox::file_name(&scope.account);
        let path = outbox::outbox_path(&self.state_dir, &name);
        let snapshot = held.clone();
        match tokio::task::spawn_blocking(move || outbox::save(&path, &snapshot)).await {
            Ok(Ok(())) => {},
            Ok(Err(error)) => {
                tracing::warn!(account = %scope.account, error = %error, "writing the outbox failed");
                return;
            },
            Err(error) => {
                tracing::warn!(account = %scope.account, error = %error, "writing the outbox did not finish");
                return;
            },
        }
        let recorded = scope.store.outbox(&scope.account).ok().flatten();
        if recorded.as_deref() != Some(name.as_str())
            && let Err(error) = scope.store.set_outbox(&scope.account, Some(name))
        {
            tracing::warn!(account = %scope.account, error = %error, "recording the outbox pointer failed");
        }
    }
}
