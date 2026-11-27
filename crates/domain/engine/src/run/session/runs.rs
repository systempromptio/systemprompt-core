//! The sessions that are running, each with its cancellation token, inbox and
//! status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use crate::store::SessionId;
use harness_tasks::CancellationToken;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use super::inbox::Inbox;
use super::{Admission, Delivery};
use crate::run::events::{EngineEvent, RunStatus, publish};

#[expect(
    variant_size_differences,
    reason = "the admission is already boxed; a pointer beside a one-byte delivery is the smallest form"
)]
pub(crate) enum Admit {
    Start(Box<Admission>),
    Inboxed(Delivery),
}

struct RunEntry {
    cancel: CancellationToken,
    inbox: Inbox,
    status: RunStatus,
}

impl RunEntry {
    fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            inbox: Inbox::default(),
            status: RunStatus::Busy,
        }
    }
}

#[derive(Default)]
pub(crate) struct Runs {
    map: Mutex<HashMap<SessionId, RunEntry>>,
}

impl Runs {
    pub(crate) fn admit(
        &self,
        session: &SessionId,
        admission: Admission,
        parent: Option<&CancellationToken>,
    ) -> Admit {
        let mut map = self.map.lock();
        if let Some(entry) = map.get_mut(session) {
            let delivery = admission.delivery;
            entry.inbox.push(admission);
            return Admit::Inboxed(delivery);
        }
        let cancel = parent.map_or_else(CancellationToken::new, CancellationToken::child_token);
        map.insert(session.clone(), RunEntry::new(cancel));
        drop(map);
        Admit::Start(Box::new(admission))
    }

    pub(crate) fn begin(&self, session: &SessionId) -> bool {
        let mut map = self.map.lock();
        if map.contains_key(session) {
            return false;
        }
        map.insert(session.clone(), RunEntry::new(CancellationToken::new()));
        drop(map);
        true
    }

    pub(crate) fn end(&self, session: &SessionId) {
        self.map.lock().remove(session);
    }

    pub(crate) fn cancel_token(&self, session: &SessionId) -> Option<CancellationToken> {
        self.map
            .lock()
            .get(session)
            .map(|entry| entry.cancel.clone())
    }

    pub(crate) fn take_steers(&self, session: &SessionId) -> Vec<Admission> {
        self.map
            .lock()
            .get_mut(session)
            .map(|entry| entry.inbox.take_steers())
            .unwrap_or_default()
    }

    pub(crate) fn finish_or_next(
        &self,
        session: &SessionId,
        events: &broadcast::Sender<EngineEvent>,
    ) -> Option<Admission> {
        let mut map = self.map.lock();
        let entry = map.get_mut(session)?;
        if let Some(next) = entry.inbox.pop() {
            if entry.cancel.is_cancelled() {
                entry.cancel = CancellationToken::new();
            }
            return Some(next);
        }
        map.remove(session);
        publish(
            events,
            EngineEvent::Status {
                session: session.clone(),
                status: RunStatus::Idle,
            },
        );
        drop(map);
        None
    }

    pub(crate) fn set_status(
        &self,
        session: &SessionId,
        status: RunStatus,
        events: &broadcast::Sender<EngineEvent>,
    ) {
        let mut map = self.map.lock();
        if let Some(entry) = map.get_mut(session) {
            entry.status = status.clone();
            publish(
                events,
                EngineEvent::Status {
                    session: session.clone(),
                    status,
                },
            );
        }
        drop(map);
    }

    pub(crate) fn status(&self, session: &SessionId) -> RunStatus {
        self.map
            .lock()
            .get(session)
            .map_or(RunStatus::Idle, |entry| entry.status.clone())
    }

    pub(crate) fn is_running(&self, session: &SessionId) -> bool {
        self.map.lock().contains_key(session)
    }

    pub(crate) fn abort(&self, session: &SessionId) -> bool {
        let mut map = self.map.lock();
        match map.get_mut(session) {
            Some(entry) => {
                entry.inbox.clear();
                entry.cancel.cancel();
                true
            },
            None => false,
        }
    }

    pub(crate) fn abort_all(&self) {
        let mut map = self.map.lock();
        for entry in map.values_mut() {
            entry.inbox.clear();
            entry.cancel.cancel();
        }
    }
}
