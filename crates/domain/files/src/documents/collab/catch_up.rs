//! Catching an edit session's cache entry up with the shared update log.
//!
//! The backlog, and a stored snapshot to reopen when another replica's
//! snapshot pruned rows this entry has not imported, are read without the
//! entry lock; the lock is then taken and the rows applied only when the
//! entry did not move meanwhile, retrying a few times before catching up
//! under the lock.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::binding::fold_actor;
use super::committer::is_write_back;
use super::entry::{CollabEntry, SharedEntry, is_unreadable};
use super::image_owners::{ImageOwners, adds_media, attribute_runs, author_runs};
use super::{CollabError, CollabService};
use crate::documents::agents::DocumentActor;
use crate::documents::engine::{EngineDoc, ModelView};
use crate::documents::repository::{CollabUpdateRow, StoredSnapshot};
use systemprompt_identifiers::DocumentEditSessionId;
use tokio::sync::MutexGuard;

const CATCH_UP_ATTEMPTS: u32 = 3;

struct Reopened {
    stored: StoredSnapshot,
    doc: Box<dyn EngineDoc>,
    images: ImageOwners,
}

struct Fetched {
    reopened: Option<Reopened>,
    rows: Vec<CollabUpdateRow>,
}

impl CollabService {
    pub(super) async fn refresh(&self, shared: &SharedEntry) -> Result<(), CollabError> {
        let entry = self.caught_up(shared).await?;
        drop(entry);
        Ok(())
    }

    pub(super) async fn catch_up(&self, entry: &mut CollabEntry) -> Result<(), CollabError> {
        match self.import_backlog(entry).await {
            Err(error) if is_unreadable(&error) => {
                Err(self.quarantine(&entry.session, &error).await)
            },
            other => other,
        }
    }

    pub(super) async fn caught_up<'a>(
        &self,
        shared: &'a SharedEntry,
    ) -> Result<MutexGuard<'a, CollabEntry>, CollabError> {
        for _ in 0..CATCH_UP_ATTEMPTS {
            let (session, from) = {
                let entry = shared.lock().await;
                (entry.session.clone(), entry.last_seq)
            };
            let fetched = match self.fetch_backlog(&session.id, from).await {
                Err(error) if is_unreadable(&error) => {
                    return Err(self.quarantine(&session, &error).await);
                },
                other => other?,
            };
            let mut entry = shared.lock().await;
            if entry.last_seq != from {
                continue;
            }
            match self.apply_backlog(&mut entry, fetched) {
                Err(error) if is_unreadable(&error) => {
                    return Err(self.quarantine(&entry.session, &error).await);
                },
                other => other?,
            }
            return Ok(entry);
        }
        let mut entry = shared.lock().await;
        self.catch_up(&mut entry).await?;
        Ok(entry)
    }

    async fn import_backlog(&self, entry: &mut CollabEntry) -> Result<(), CollabError> {
        let fetched = self
            .fetch_backlog(&entry.session.id, entry.last_seq)
            .await?;
        self.apply_backlog(entry, fetched)
    }

    async fn fetch_backlog(
        &self,
        session: &DocumentEditSessionId,
        after_seq: i64,
    ) -> Result<Fetched, CollabError> {
        let backlog = self
            .documents
            .repositories()
            .collab
            .backlog(session, after_seq)
            .await?;
        let reopened = match backlog.snapshot {
            Some(stored) => {
                let (doc, images) = self.open_snapshot(&stored.hash).await?;
                Some(Reopened {
                    stored,
                    doc,
                    images,
                })
            },
            None => None,
        };
        Ok(Fetched {
            reopened,
            rows: backlog.rows,
        })
    }

    fn apply_backlog(&self, entry: &mut CollabEntry, fetched: Fetched) -> Result<(), CollabError> {
        let Fetched { reopened, rows } = fetched;
        if let Some(Reopened {
            stored,
            doc,
            images,
        }) = reopened
        {
            entry.doc = doc;
            entry.images = images;
            entry.last_seq = stored.seq;
            entry.session.snapshot_sha256 = Some(stored.hash);
        }
        let rows: Vec<CollabUpdateRow> = rows
            .into_iter()
            .filter(|row| row.seq > entry.last_seq)
            .collect();
        if rows.is_empty() {
            return Ok(());
        }
        let engine = self.engine()?;
        let before = entry.doc.export_model()?;
        let replay = (author_runs(&rows) > 1).then(|| entry.doc.fork());
        for row in &rows {
            engine.check_update(&row.update, None)?;
            entry.doc.apply_update(&row.update)?;
            entry.last_seq = entry.last_seq.max(row.seq);
            if !is_write_back(row.actor.as_ref()) {
                entry.last_actor = Some(fold_actor(
                    entry.last_actor.as_ref(),
                    row.actor.clone().unwrap_or(DocumentActor::Human),
                ));
            }
        }
        let after = entry.doc.export_model()?;
        let (ModelView::RichText(prior), ModelView::RichText(next)) = (&before, &after) else {
            return Ok(());
        };
        if !adds_media(prior, next) {
            return Ok(());
        }
        match replay {
            None => entry.images.record(prior, next, &rows[0].author_user_id),
            Some(fork) => attribute_runs(&mut entry.images, fork, before, &rows)?,
        }
        Ok(())
    }
}
