//! The facts the instance's gateway injected into each session's latest model
//! call, read from the `x-systemprompt-memory-ids` response header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::engine::SessionId;
use parking_lot::Mutex;
use sp_protocol::wire::brain::FactRef;

#[derive(Debug, Default)]
pub(crate) struct MemoryLedger {
    sessions: Mutex<HashMap<SessionId, Vec<FactRef>>>,
}

impl MemoryLedger {
    pub(crate) fn get(&self, session: &SessionId) -> Vec<FactRef> {
        self.sessions
            .lock()
            .get(session)
            .cloned()
            .unwrap_or_else(Vec::new)
    }

    pub(crate) fn record(&self, session: &SessionId, facts: Vec<FactRef>) -> bool {
        let mut sessions = self.sessions.lock();
        match sessions.get_mut(session) {
            Some(held) if *held == facts => false,
            Some(held) => {
                *held = facts;
                true
            },
            None if facts.is_empty() => false,
            None => {
                sessions.insert(session.clone(), facts);
                true
            },
        }
    }

    pub(crate) fn forget(&self, session: &SessionId) -> bool {
        self.sessions.lock().remove(session).is_some()
    }

    pub(crate) fn clear(&self) -> bool {
        let mut sessions = self.sessions.lock();
        let held = !sessions.is_empty();
        sessions.clear();
        held
    }
}
