//! The session directories governance remembers for its event bodies, and the
//! hand-off of one event to the owned sender.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_identifiers::engine::SessionId;
use systemprompt_models::bridge::hook_events::HookEvent;

use super::Governance;
use crate::governance::instance::Instance;
use crate::governance::telemetry::{Job, Rejected};

const REMEMBERED_SESSIONS: usize = 4_096;

impl Governance {
    pub(super) fn remember_directory(&self, session: &str, directory: &Path) {
        let mut sessions = self.shared.sessions.lock();
        if sessions
            .get(session)
            .is_some_and(|known| known == directory)
        {
            return;
        }
        sessions.insert(session.to_owned(), directory.to_path_buf());
        if sessions.len() > REMEMBERED_SESSIONS {
            let _evicted = sessions.shift_remove_index(0);
        }
    }

    pub(super) fn directory(&self, session: &str) -> PathBuf {
        self.shared
            .sessions
            .lock()
            .get(session)
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn forget_directory(&self, session: &str) -> PathBuf {
        self.shared
            .sessions
            .lock()
            .shift_remove(session)
            .unwrap_or_default()
    }

    pub(super) fn report(
        &self,
        native_session: &SessionId,
        build: impl FnOnce(&Instance) -> Option<HookEvent>,
    ) {
        let Some(instance) = self.current() else {
            return;
        };
        let Some(plugin) = instance.governance_plugin().cloned() else {
            return;
        };
        let Some(event) = build(&instance) else {
            return;
        };
        let name = event.name();
        let Some(events) = &self.events else {
            tracing::debug!(
                event = name,
                "governance event dropped: no sender is running"
            );
            return;
        };
        let job = Job {
            instance,
            plugin,
            native_session: native_session.clone(),
            event,
        };
        if let Err(error) = events.submit(job) {
            let full = error == Rejected::Full;
            tracing::debug!(
                event = name,
                full,
                "governance event dropped: the queue is not accepting"
            );
        }
    }
}
