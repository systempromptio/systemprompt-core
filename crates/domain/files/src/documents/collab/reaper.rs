//! Settling edit sessions no replica is driving any more.
//!
//! A session whose last update, snapshot or checkpoint is older than the
//! idle window, with no live web presence and no unexpired checkpoint lease,
//! was left by editors whose leave never settled it (a replica restarted, a
//! client vanished, a leave lost a race). [`CollabService::reap_idle`] settles
//! a bounded batch of them as the most recent author the update log records
//! (see the `committer` module), never as a user who did not write to the
//! session, and closes them; a session whose document is gone or
//! trashed, or whose format is not co-edited, is closed without a checkpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::TraceId;

use super::lease::SettledBy;
use super::model::is_collab_format;
use super::{CollabError, CollabService};
use crate::documents::models::DocumentEditSession;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reaped {
    pub closed: u64,
    pub left_open: u64,
    pub failed: u64,
}

impl CollabService {
    pub async fn reap_idle(&self, idle: Duration, limit: i64) -> Result<Reaped, CollabError> {
        let stale = self
            .documents
            .repositories()
            .edit_sessions
            .list_stale(idle, limit)
            .await?;
        let mut reaped = Reaped::default();
        for session in stale {
            match self.reap_one(&session).await {
                Ok(true) => reaped.closed += 1,
                Ok(false) => reaped.left_open += 1,
                Err(error) => {
                    reaped.failed += 1;
                    tracing::warn!(
                        document_id = %session.document_id,
                        session_id = %session.id,
                        error = %error,
                        "Idle edit session could not be settled"
                    );
                },
            }
        }
        Ok(reaped)
    }

    async fn reap_one(&self, session: &DocumentEditSession) -> Result<bool, CollabError> {
        let repos = self.documents.repositories();
        let document = repos.documents.find(&session.document_id).await?;
        let Some(document) = document.filter(|document| is_collab_format(document.format)) else {
            self.cache_remove(&session.document_id);
            return Ok(repos.edit_sessions.close(&session.id).await?);
        };
        let entry = self.entry(&document, session).await?;
        let left = self
            .settle_as(&entry, None, &TraceId::system(), SettledBy::Reaper)
            .await?;
        if left.closed {
            self.cache_remove(&document.id);
        }
        Ok(left.closed)
    }
}
