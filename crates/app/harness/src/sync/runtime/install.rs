//! One capture's installations: each projected skill publication is skipped
//! while the outbox holds a receipt for exactly it and its files are still on
//! disk; otherwise its plan is read with the device credential, written and
//! read back by the applier, and turned into a receipt. The whole pass has 30
//! s; none starts with less than 2 s left. A failed installation waits like a
//! receipt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::projection::SkillPublication;
use systemprompt_device::client::ConsumerOutcome;
use systemprompt_device::protocol::receipts::{build_receipt, plan_matches};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::feedback::receipts::{ConsumerInstallationPlan, ConsumerReceiptRequest};
use tokio::time::Instant;

use super::device::HeldDevice;
use super::{RuntimeFiles, WaitKey};
use crate::sync::cadence::{INSTALL_BUDGET, INSTALL_MINIMUM, outbox_delay};
use crate::sync::outbox::Outbox;
use crate::sync::poll::Scope;
use crate::sync::ports::{Instance, Store};
use crate::sync::target::Target;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Installed {
    pub(super) written: usize,
    pub(super) remaining: usize,
    pub(super) device_refused: bool,
}

enum Installation {
    Receipt(Box<(ConsumerReceiptRequest, ConsumerInstallationPlan)>),
    Intact(Box<ConsumerInstallationPlan>),
    DeviceRefused,
    Failed,
}

impl RuntimeFiles {
    pub(super) async fn install_all<S: Store, T: Target>(
        &mut self,
        scope: &Scope<S, T>,
        device: &HeldDevice,
        outbox: &mut Outbox,
        publications: &[SkillPublication],
    ) -> Installed {
        let deadline = Instant::now() + INSTALL_BUDGET;
        let mut tally = Installed::default();
        for (position, publication) in publications.iter().enumerate() {
            let holds = outbox.holds(&publication.publication);
            if holds && self.intact(scope, publication).await {
                continue;
            }
            let key = WaitKey::Publication(publication.publication.publication_id.clone());
            let left = deadline.saturating_duration_since(Instant::now());
            if !self.due(&key, scope.clock.now()) || left < INSTALL_MINIMUM {
                tally.remaining += 1;
                continue;
            }
            let installation =
                tokio::time::timeout(left, install(scope, device, publication, holds))
                    .await
                    .unwrap_or(Installation::Failed);
            match installation {
                Installation::Receipt(boxed) => {
                    let (receipt, plan) = *boxed;
                    outbox.enqueue(receipt, scope.clock.now());
                    self.plans.insert(plan.publication_id.clone(), plan);
                    self.waits.remove(&key);
                    tally.written += 1;
                },
                Installation::Intact(plan) => {
                    self.plans.insert(plan.publication_id.clone(), *plan);
                    self.waits.remove(&key);
                },
                Installation::DeviceRefused => {
                    tally.remaining += publications.len() - position;
                    tally.device_refused = true;
                    return tally;
                },
                Installation::Failed => {
                    self.failed(key, scope.clock.now(), outbox_delay);
                    tally.remaining += 1;
                },
            }
        }
        tally
    }

    async fn intact<S: Store, T: Target>(
        &self,
        scope: &Scope<S, T>,
        publication: &SkillPublication,
    ) -> bool {
        let Some(plan) = self
            .plans
            .get(&publication.publication.publication_id)
            .filter(|plan| plan_matches(plan, &publication.publication, HostKind::Harness))
        else {
            return false;
        };
        scope
            .target
            .inspect(publication.dir.clone(), plan.clone())
            .await
    }
}

async fn install<S: Store, T: Target>(
    scope: &Scope<S, T>,
    device: &HeldDevice,
    publication: &SkillPublication,
    holds: bool,
) -> Installation {
    let plan = match scope
        .instance
        .installation_plan(&device.credential, &publication.publication)
        .await
    {
        ConsumerOutcome::Acknowledged(plan) => plan,
        ConsumerOutcome::DeviceRefused(_) => return Installation::DeviceRefused,
        ConsumerOutcome::Refused(error) | ConsumerOutcome::RetryLater(error) => {
            tracing::warn!(skill = %publication.dir, error = %error, "the installation plan could not be read");
            return Installation::Failed;
        },
    };
    if holds
        && scope
            .target
            .inspect(publication.dir.clone(), plan.clone())
            .await
    {
        return Installation::Intact(Box::new(plan));
    }
    let readback = match scope
        .target
        .materialize(publication.dir.clone(), plan.clone())
        .await
    {
        Ok(readback) => readback,
        Err(error) => {
            tracing::warn!(skill = %publication.dir, error = %error, "the runtime files could not be written");
            return Installation::Failed;
        },
    };
    match build_receipt(
        &plan,
        &readback,
        device.installation.clone(),
        scope.clock.now(),
    ) {
        Ok(receipt) => Installation::Receipt(Box::new((receipt, plan))),
        Err(mismatch) => {
            tracing::warn!(skill = %publication.dir, error = %mismatch, "the runtime files did not read back as planned");
            Installation::Failed
        },
    }
}
