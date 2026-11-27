//! Sending governance events and asking for verdicts.
//!
//! Events go through a bounded queue to one owned sender task, in the order
//! they happened. Each send has five seconds for its hook token and its call
//! together, is never retried and never reaches back into the engine: a full
//! queue, a failure or a timeout drops the event with a debug line. A verdict
//! has three seconds; anything but an explicit deny inside that budget lets the
//! tool run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_traits::{CancellationToken, OwnedTask};
use systemprompt_device::protocol::names::native_session_id;
use systemprompt_engine::run::Verdict;
use systemprompt_identifiers::PluginId;
use systemprompt_identifiers::engine::SessionId;
use systemprompt_models::bridge::hook_events::{HOOK_TIMEOUT_MS, HookEvent, VERDICT_TIMEOUT_MS};
use tokio::sync::mpsc;

use crate::governance::error::GovernanceError;
use crate::governance::governance::{SessionBinding, Shared};
use crate::governance::hooks::hook_token;
use crate::governance::instance::Instance;

const QUEUE: usize = 256;

const HOOK_BUDGET: Duration = Duration::from_millis(HOOK_TIMEOUT_MS);

const VERDICT_BUDGET: Duration = Duration::from_millis(VERDICT_TIMEOUT_MS);

pub(crate) struct Job {
    pub(crate) instance: Arc<Instance>,
    pub(crate) plugin: PluginId,
    pub(crate) native_session: SessionId,
    pub(crate) event: HookEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rejected {
    Full,
    Closed,
}

pub(crate) struct EventQueue {
    jobs: mpsc::Sender<Job>,
    sender: OwnedTask<()>,
}

impl EventQueue {
    pub(crate) fn start(shared: Arc<Shared>, cancel: CancellationToken) -> Option<Self> {
        if tokio::runtime::Handle::try_current().is_err() {
            tracing::warn!(
                "governance built outside the async runtime; governance events will not be sent"
            );
            return None;
        }
        let (jobs, receiver) = mpsc::channel(QUEUE);
        let sender = OwnedTask::spawn("governance-events", run(shared, receiver, cancel));
        Some(Self { jobs, sender })
    }

    pub(crate) fn submit(&self, job: Job) -> Result<(), Rejected> {
        self.jobs.try_send(job).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => Rejected::Full,
            mpsc::error::TrySendError::Closed(_) => Rejected::Closed,
        })
    }

    pub(crate) fn stop(&self) {
        self.sender.abort();
    }
}

async fn run(shared: Arc<Shared>, mut jobs: mpsc::Receiver<Job>, cancel: CancellationToken) {
    loop {
        let job = tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            job = jobs.recv() => match job {
                Some(job) => job,
                None => break,
            },
        };
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            () = deliver(&shared, job) => {}
        }
    }
    tracing::debug!("governance event sender stopped");
}

async fn deliver(shared: &Shared, job: Job) {
    if shared.generation() != job.instance.epoch {
        tracing::debug!(
            event = job.event.name(),
            "governance event dropped: the instance changed"
        );
        return;
    }
    let device = shared.device();
    let name = job.event.name();
    let sent = tokio::time::timeout(HOOK_BUDGET, async {
        let token = hook_token(&job.instance, &job.plugin).await?;
        job.instance
            .api
            .send_event(&token, &job.plugin, &job.event, device.as_ref())
            .await
            .map_err(GovernanceError::Hook)
    })
    .await;
    match sent {
        Ok(Ok(())) => {
            tracing::trace!(event = name, "governance event sent");
            if device.is_some() {
                bind(shared, &job);
            }
        },
        Ok(Err(error)) => tracing::debug!(event = name, error = %error, "governance event dropped"),
        Err(_elapsed) => tracing::debug!(
            event = name,
            "governance event dropped: the instance did not answer in time"
        ),
    }
}

fn bind(shared: &Shared, job: &Job) {
    if !job.instance.first_binding(&job.native_session) {
        return;
    }
    let binding = SessionBinding {
        instance_url: job.instance.applied.instance_url.clone(),
        session_id: native_session_id(job.native_session.as_str()),
        native_session_id: job.native_session.clone(),
    };
    if shared.bindings.send(binding).is_err() {
        tracing::debug!("a session binding found no listener");
    }
}

pub(crate) async fn verdict(instance: &Instance, plugin: &PluginId, event: &HookEvent) -> Verdict {
    let asked = tokio::time::timeout(VERDICT_BUDGET, async {
        let token = hook_token(instance, plugin).await?;
        instance
            .api
            .ask_verdict(&token, plugin, event)
            .await
            .map_err(GovernanceError::Hook)
    })
    .await;
    match asked {
        Ok(Ok(answer)) => answer
            .refusal()
            .map_or(Verdict::Allow, |reason| Verdict::Deny {
                reason: reason.to_owned(),
            }),
        Ok(Err(error)) => {
            tracing::debug!(error = %error, "no verdict from the instance; the tool runs");
            Verdict::Allow
        },
        Err(_elapsed) => {
            tracing::debug!(
                budget_ms = VERDICT_TIMEOUT_MS,
                "no verdict in time; the tool runs"
            );
            Verdict::Allow
        },
    }
}
