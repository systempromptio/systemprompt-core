//! [`BrainService`]: the brain of the applied instance, as the shell, the
//! agent tools and the call hooks share it.
//!
//! The service follows the applied instance through the extension's
//! [`ManifestApplier`](sp_extension::ManifestApplier): an instance that admits
//! the extension attaches its client and the service reads
//! [`BrainStatus::Checking`] until [`BrainProbe`] has asked the instance
//! whether its brain is enabled. It fails closed: only
//! [`BrainStatus::Enabled`] lets a call through; a detached, unconfirmed or
//! disabled brain answers its error without a request, and an answer saying
//! the brain is disabled withholds it from then on. Every change is published
//! on one feed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod actions;
mod probe;

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use harness_ids::SessionId;
use parking_lot::Mutex;
use sp_client::HttpClient;
use sp_protocol::wire::brain::FactRef;
use tokio::sync::watch;

pub use probe::BrainProbe;

use crate::client::BrainClient;
use crate::error::BrainError;
use crate::memory::MemoryLedger;

/// Whether the brain of the applied instance can be used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BrainStatus {
    #[default]
    Detached,
    Checking,
    Enabled,
    Disabled,
}

impl BrainStatus {
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// What the brain looks like now: its status and one counter per kind of
/// change (facts governed or entered, proposals decided, memory recorded for
/// a session), each bumped when that kind changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrainFeed {
    pub status: BrainStatus,
    pub facts: u64,
    pub proposals: u64,
    pub memory: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Nothing,
    Facts,
    Proposals,
}

/// The brain of the applied instance. Cheap to clone; every clone shares the
/// attached client, the feed and the per-session memory record.
#[derive(Clone)]
pub struct BrainService {
    inner: Arc<Inner>,
}

struct Inner {
    http: HttpClient,
    attached: Mutex<Attached>,
    attachments: watch::Sender<u64>,
    feed: watch::Sender<BrainFeed>,
    memory: MemoryLedger,
}

#[derive(Default)]
struct Attached {
    epoch: u64,
    client: Option<BrainClient>,
}

impl BrainService {
    #[must_use]
    pub fn new(http: HttpClient) -> Self {
        Self {
            inner: Arc::new(Inner {
                http,
                attached: Mutex::new(Attached::default()),
                attachments: watch::Sender::new(0),
                feed: watch::Sender::new(BrainFeed::default()),
                memory: MemoryLedger::default(),
            }),
        }
    }

    #[must_use]
    pub fn http(&self) -> &HttpClient {
        &self.inner.http
    }

    pub fn attach(&self, client: BrainClient) {
        let epoch = {
            let mut attached = self.inner.attached.lock();
            attached.epoch += 1;
            attached.client = Some(client);
            attached.epoch
        };
        self.set_status(BrainStatus::Checking);
        self.inner.attachments.send_replace(epoch);
    }

    pub fn detach(&self) {
        let epoch = {
            let mut attached = self.inner.attached.lock();
            attached.epoch += 1;
            attached.client = None;
            attached.epoch
        };
        let forgot = self.inner.memory.clear();
        self.inner.feed.send_modify(|feed| {
            feed.status = BrainStatus::Detached;
            if forgot {
                feed.memory += 1;
            }
        });
        self.inner.attachments.send_replace(epoch);
    }

    #[must_use]
    pub fn status(&self) -> BrainStatus {
        self.inner.feed.borrow().status
    }

    #[must_use]
    pub fn feed(&self) -> BrainFeed {
        *self.inner.feed.borrow()
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<BrainFeed> {
        self.inner.feed.subscribe()
    }

    #[must_use]
    pub fn memory(&self, session: &SessionId) -> Vec<FactRef> {
        self.inner.memory.get(session)
    }

    pub(crate) fn record_memory(&self, session: &SessionId, facts: Vec<FactRef>) {
        if self.inner.memory.record(session, facts) {
            self.inner.feed.send_modify(|feed| feed.memory += 1);
        }
    }

    pub(crate) fn forget_memory(&self, session: &SessionId) {
        if self.inner.memory.forget(session) {
            self.inner.feed.send_modify(|feed| feed.memory += 1);
        }
    }

    fn attachments(&self) -> watch::Receiver<u64> {
        self.inner.attachments.subscribe()
    }

    fn pending_probe(&self) -> Option<(BrainClient, u64)> {
        if self.status() != BrainStatus::Checking {
            return None;
        }
        let attached = self.inner.attached.lock();
        attached
            .client
            .clone()
            .map(|client| (client, attached.epoch))
    }

    fn settle(&self, epoch: u64, status: BrainStatus) {
        let current = {
            let attached = self.inner.attached.lock();
            attached.epoch == epoch && attached.client.is_some()
        };
        if current {
            tracing::info!(status = ?status, "brain availability read");
            self.set_status(status);
        }
    }

    fn set_status(&self, status: BrainStatus) {
        self.inner.feed.send_if_modified(|feed| {
            let changed = feed.status != status;
            feed.status = status;
            changed
        });
    }

    fn usable(&self) -> Result<(BrainClient, u64), BrainError> {
        match self.status() {
            BrainStatus::Enabled => {},
            BrainStatus::Disabled => return Err(BrainError::Disabled),
            BrainStatus::Checking => return Err(BrainError::Unconfirmed),
            BrainStatus::Detached => return Err(BrainError::NoInstance),
        }
        let attached = self.inner.attached.lock();
        attached
            .client
            .clone()
            .map(|client| (client, attached.epoch))
            .ok_or(BrainError::NoInstance)
    }

    async fn call<T, F, Fut>(&self, change: Change, run: F) -> Result<T, BrainError>
    where
        F: FnOnce(BrainClient) -> Fut,
        Fut: Future<Output = Result<T, BrainError>>,
    {
        let (client, epoch) = self.usable()?;
        let result = run(client).await;
        match (&result, change) {
            (Err(BrainError::Disabled), _) => self.settle(epoch, BrainStatus::Disabled),
            (Ok(_), Change::Facts) => self.inner.feed.send_modify(|feed| feed.facts += 1),
            (Ok(_), Change::Proposals) => self.inner.feed.send_modify(|feed| feed.proposals += 1),
            _ => {},
        }
        result
    }
}

impl fmt::Debug for BrainService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrainService")
            .field("feed", &*self.inner.feed.borrow())
            .finish_non_exhaustive()
    }
}
