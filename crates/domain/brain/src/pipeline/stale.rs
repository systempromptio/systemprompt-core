//! The stale stage, run before the filters so a source change stales the
//! facts built on what it replaced even when nothing new is gathered.
//!
//! A landed revision stales the facts of earlier revisions of its document,
//! and a resolved comment stales the facts built on it. Each mark is a
//! `Staled` revision written by the repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::BrainFactId;

use super::normalise::{EventPayload, Normalised};
use crate::error::BrainResult;
use crate::repository::FactRepository;

pub async fn mark_stale(
    conn: &mut PgConnection,
    event: &Normalised,
) -> BrainResult<Vec<BrainFactId>> {
    let staled = match &event.payload {
        EventPayload::Revision(landed) => {
            FactRepository::mark_stale_for_document(conn, &landed.document_id, &landed.revision_id)
                .await?
        },
        EventPayload::Comment(resolved) => {
            FactRepository::mark_stale_for_comment(conn, &resolved.comment_id).await?
        },
        EventPayload::Entered(_) => Vec::new(),
    };
    if !staled.is_empty() {
        tracing::debug!(
            outbox_id = %event.envelope.outbox_id,
            staled = staled.len(),
            "Brain facts staled by a source change"
        );
    }
    Ok(staled)
}
