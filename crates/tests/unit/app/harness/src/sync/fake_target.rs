use std::future::Future;
use std::sync::{Arc, Mutex};
use systemprompt_models::feedback::receipts::{ConsumerInstallationPlan, RuntimeFileReadback};

use systemprompt_device::protocol::DeviceCredential;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_harness::extension::EnabledExtensions;
use systemprompt_harness::governance::{SessionBinding, UsageTotals};
use systemprompt_harness::projection::{Applied, ApplyReport, Counts, Projection};
use tokio::sync::broadcast;

use systemprompt_harness::sync::target::{Target, TargetError};

use crate::sync::fakes::readback;

#[derive(Default)]
pub(crate) struct TargetState {
    pub(crate) applies: usize,
    pub(crate) failing_applies: usize,
    pub(crate) governed: usize,
    pub(crate) enabled: Vec<Option<EnabledExtensions>>,
    pub(crate) clears: usize,
    pub(crate) materialized: usize,
    pub(crate) intact: bool,
    pub(crate) device: Option<String>,
    pub(crate) skills: u32,
}

#[derive(Clone)]
pub(crate) struct FakeTarget {
    pub(crate) state: Arc<Mutex<TargetState>>,
    pub(crate) bindings: broadcast::Sender<SessionBinding>,
}

impl Default for FakeTarget {
    fn default() -> Self {
        Self {
            state: Arc::default(),
            bindings: broadcast::channel(16).0,
        }
    }
}

impl FakeTarget {
    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, TargetState> {
        self.state.lock().unwrap()
    }
}

impl Target for FakeTarget {
    fn apply(
        &self,
        projection: Projection,
    ) -> impl Future<Output = Result<ApplyReport, TargetError>> + Send {
        let failing = {
            let mut state = self.state();
            if state.failing_applies > 0 {
                state.failing_applies -= 1;
                true
            } else {
                state.applies += 1;
                state.skills = projection.counts.skills;
                false
            }
        };
        let answer = if failing {
            Err(TargetError::Projection(
                systemprompt_harness::projection::ProjectionError::Unowned {
                    path: "x".to_owned(),
                },
            ))
        } else {
            Ok(ApplyReport {
                counts: Counts {
                    skills: projection.counts.skills,
                    ..Counts::default()
                },
                warnings: Vec::new(),
            })
        };
        std::future::ready(answer)
    }

    fn clear(&self) -> impl Future<Output = ()> + Send {
        self.state().clears += 1;
        std::future::ready(())
    }

    fn govern(
        &self,
        _account: &AccountId,
        _applied: Applied,
        extensions: Option<EnabledExtensions>,
    ) -> Result<(), TargetError> {
        {
            let mut state = self.state();
            state.governed += 1;
            state.enabled.push(extensions);
        }
        Ok(())
    }

    fn materialize(
        &self,
        _skill_dir: String,
        plan: ConsumerInstallationPlan,
    ) -> impl Future<Output = Result<Vec<RuntimeFileReadback>, TargetError>> + Send {
        let answer = {
            self.state().materialized += 1;
            Ok(readback(&plan))
        };
        std::future::ready(answer)
    }

    fn inspect(
        &self,
        _skill_dir: String,
        _plan: ConsumerInstallationPlan,
    ) -> impl Future<Output = bool> + Send {
        std::future::ready(self.state().intact)
    }

    fn set_device(&self, device: Option<DeviceCredential>) {
        self.state().device = device.map(|credential| credential.expose().to_owned());
    }

    fn usage(&self) -> UsageTotals {
        UsageTotals {
            forwarded_total: 3,
            tokens_in_total: 100,
            tokens_out_total: 50,
            last_activity_at: None,
        }
    }

    fn session_bindings(&self) -> broadcast::Receiver<SessionBinding> {
        self.bindings.subscribe()
    }
}
