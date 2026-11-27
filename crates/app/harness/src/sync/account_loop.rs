//! The loop of one account: the first poll at once, then a poll whenever the
//! planner says, on focus gain and on Sync now; polls never overlap, and what
//! was asked for during one runs once after it. Session bindings governance
//! reports for this instance are handed to the runtime files between polls. The
//! loop ends when its token is cancelled or the manifest key changed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use sp_governance::SessionBinding;
use sp_protocol::ids::AccountId;
use sp_protocol::view::{SyncErrorKind, SyncStatus};
use tokio::sync::{broadcast, watch};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::bundles::BundleCache;
use crate::cadence::{Clock, RandomJitter};
use crate::planner::{Next, Planner, Trigger};
use crate::poll::{Poller, Scope};
use crate::ports::Store;
use crate::runtime::RuntimeFiles;
use crate::target::Target;

#[derive(Clone)]
pub(crate) struct LoopContext<S: Store, T: Target> {
    pub(crate) store: S,
    pub(crate) target: T,
    pub(crate) status: Arc<watch::Sender<SyncStatus>>,
    pub(crate) focus: watch::Receiver<bool>,
    pub(crate) sync_now: watch::Receiver<u64>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) state_dir: PathBuf,
    pub(crate) cache_dir: PathBuf,
    pub(crate) device_label: String,
}

pub(crate) async fn run<S: Store, T: Target>(
    context: LoopContext<S, T>,
    account: AccountId,
    cancel: CancellationToken,
) {
    let instance = match context.store.instance(&account) {
        Ok(instance) => instance,
        Err(error) => {
            tracing::warn!(account = %account, error = %error, "the account's instance cannot be reached");
            let at = context.clock.now();
            context.status.send_replace(SyncStatus::Error {
                account,
                kind: SyncErrorKind::Invalid,
                at,
            });
            return;
        },
    };
    let scope = Scope {
        account: account.clone(),
        store: context.store.clone(),
        target: context.target.clone(),
        instance,
        clock: Arc::clone(&context.clock),
        status: Arc::clone(&context.status),
    };
    let bundles = BundleCache::new(&context.cache_dir);
    let runtime = RuntimeFiles::new(&context.state_dir, &context.device_label);
    let poller = Poller::new(scope, bundles, runtime);
    tracing::info!(account = %account, "sync loop started");
    drive(context, poller, cancel).await;
    tracing::info!(account = %account, "sync loop stopped");
}

async fn drive<S: Store, T: Target>(
    context: LoopContext<S, T>,
    mut poller: Poller<S, T>,
    cancel: CancellationToken,
) {
    let mut planner = Planner::new();
    let mut focus = context.focus.clone();
    let mut focused = *focus.borrow_and_update();
    let mut sync_now = context.sync_now.clone();
    let _seen: u64 = *sync_now.borrow_and_update();
    let mut bindings = context.target.session_bindings();
    let mut bindings_open = true;
    let mut jitter = RandomJitter;
    let mut wake: Option<Instant> = None;
    loop {
        if let Some(request) = planner.take() {
            let outcome = tokio::select! {
                () = cancel.cancelled() => return,
                outcome = poller.poll(request, &mut planner) => outcome,
            };
            match planner.finished(outcome, focused, &mut jitter) {
                Next::Wait(delay) => wake = Some(Instant::now() + delay),
                Next::Halt => return,
            }
            continue;
        }
        let deadline = wake.unwrap_or_else(Instant::now);
        tokio::select! {
            () = cancel.cancelled() => return,
            () = tokio::time::sleep_until(deadline), if wake.is_some() => {
                wake = None;
                planner.request(Trigger::Timer);
            },
            changed = focus.changed() => {
                if changed.is_err() {
                    return;
                }
                let now = *focus.borrow_and_update();
                if now && !focused {
                    planner.request(Trigger::Focus);
                }
                focused = now;
            },
            changed = sync_now.changed() => {
                if changed.is_err() {
                    return;
                }
                let _asked: u64 = *sync_now.borrow_and_update();
                planner.request(Trigger::SyncNow);
            },
            received = bindings.recv(), if bindings_open => {
                match received {
                    Ok(binding) => {
                        let bound = tokio::select! {
                            () = cancel.cancelled() => return,
                            bound = bind(&poller, &binding) => bound,
                        };
                        if bound {
                            poller.mark_pending();
                        }
                    },
                    Err(broadcast::error::RecvError::Lagged(missed)) => {
                        tracing::debug!(missed, "session bindings were missed");
                    },
                    Err(broadcast::error::RecvError::Closed) => bindings_open = false,
                }
            },
        }
    }
}

async fn bind<S: Store, T: Target>(poller: &Poller<S, T>, binding: &SessionBinding) -> bool {
    if binding.instance_url != poller.applied_url() {
        return false;
    }
    poller
        .runtime
        .bind(&poller.scope, &binding.session_id)
        .await
}
