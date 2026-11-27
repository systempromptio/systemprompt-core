//! Follows the accounts: one loop for the active signed-in account. Another
//! account (or none) stops the loop, clears the managed content and governance,
//! and starts the new account's loop at once; the same account with another URL
//! or a newly trusted key restarts its loop and keeps its content until the
//! next apply. Nobody signed in at start clears what an earlier run left.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_device::accounts::AccountsSnapshot;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::{AccountStatus, AccountView, KeyFingerprint, SyncStatus};
use systemprompt_traits::OwnedTask;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::sync::account_loop::{self, LoopContext};
use crate::sync::ports::Store;
use crate::sync::target::Target;

pub(crate) fn active_signed_in(snapshot: &AccountsSnapshot) -> Option<&AccountView> {
    snapshot
        .active_account()
        .filter(|view| view.status == AccountStatus::SignedIn)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopKey {
    pub(crate) account: AccountId,
    url: String,
    pinned: Option<KeyFingerprint>,
}

impl LoopKey {
    pub(crate) fn of(snapshot: &AccountsSnapshot) -> Option<Self> {
        active_signed_in(snapshot).map(|view| Self {
            account: view.id.clone(),
            url: view.url.clone(),
            pinned: view.pinned.clone(),
        })
    }
}

struct Running {
    key: LoopKey,
    cancel: CancellationToken,
    task: OwnedTask<()>,
}

impl Running {
    async fn stop(self) -> LoopKey {
        self.cancel.cancel();
        if let Err(error) = self.task.join_named().await {
            tracing::warn!(error = %error, "a sync loop ended abnormally");
        }
        self.key
    }
}

struct Loops {
    running: Option<Running>,
    first: bool,
}

pub(crate) struct Supervisor<S: Store, T: Target> {
    pub(crate) context: LoopContext<S, T>,
    pub(crate) snapshots: watch::Receiver<AccountsSnapshot>,
}

impl<S: Store, T: Target> Supervisor<S, T> {
    pub(crate) async fn run(mut self, cancel: CancellationToken) {
        let mut loops = Loops {
            running: None,
            first: true,
        };
        loop {
            let next = LoopKey::of(&self.snapshots.borrow_and_update());
            self.reconcile(&mut loops, next, &cancel).await;
            loops.first = false;
            tokio::select! {
                () = cancel.cancelled() => break,
                changed = self.snapshots.changed() => {
                    if changed.is_err() {
                        break;
                    }
                },
            }
        }
        if let Some(current) = loops.running.take() {
            current.stop().await;
        }
        tracing::debug!("sync supervisor stopped");
    }

    async fn reconcile(
        &self,
        loops: &mut Loops,
        next: Option<LoopKey>,
        cancel: &CancellationToken,
    ) {
        let unchanged = matches!((&loops.running, &next), (Some(current), Some(wanted)) if current.key == *wanted);
        if unchanged {
            return;
        }
        match loops.running.take() {
            Some(previous) => {
                let previous = previous.stop().await;
                let switched = next
                    .as_ref()
                    .is_none_or(|wanted| wanted.account != previous.account);
                if switched {
                    tracing::info!(
                        account = %previous.account,
                        "the synced account changed; clearing its content"
                    );
                    self.context.target.clear().await;
                }
            },
            None if loops.first && next.is_none() => self.context.target.clear().await,
            None => {},
        }
        let Some(key) = next else {
            self.context.status.send_replace(SyncStatus::Idle);
            return;
        };
        let token = cancel.child_token();
        let run = account_loop::run(self.context.clone(), key.account.clone(), token.clone());
        let task = OwnedTask::spawn("sync-account-loop", run);
        loops.running = Some(Running {
            key,
            cancel: token,
            task,
        });
    }
}
