//! The fork-side check of one client update before it is logged: import on
//! a fork of the live document, refuse parked operations and unassigned
//! peers, then validate what changed at the caller's level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CollabError;
use crate::documents::engine::{EngineDoc, EngineError, ImportScope, ModelView};
use crate::documents::model::{EditLevel, Limits, ModelRejection, RichText, validate};
use crate::documents::repository::UpdateAuthor;

pub(super) struct Candidate<'a> {
    pub(super) update: &'a [u8],
    pub(super) level: EditLevel,
    pub(super) peers: &'a [u64],
    pub(super) author: UpdateAuthor<'a>,
}

pub(super) async fn check_on_fork(
    mut draft: Box<dyn EngineDoc>,
    candidate: &Candidate<'_>,
) -> Result<(), CollabError> {
    let update = candidate.update.to_vec();
    let scoped = candidate.level == EditLevel::Edit;
    let imported =
        tokio::task::spawn_blocking(move || draft.apply_update_scoped(&update, scoped)).await;
    let scoped = match imported {
        Ok(result) => result.map_err(CollabError::InvalidUpdate)?,
        Err(error) => {
            tracing::warn!(
                error = %error,
                panicked = error.is_panic(),
                "Collab update import did not complete on its fork"
            );
            return Err(CollabError::InvalidUpdate(EngineError::State(Box::new(
                error,
            ))));
        },
    };
    let import = scoped.import;
    if import.pending {
        return Err(CollabError::MissingDependencies);
    }
    if let Some(peer) = import
        .peers
        .iter()
        .find(|peer| !candidate.peers.contains(peer))
    {
        return Err(CollabError::PeerNotAssigned { peer: *peer });
    }
    match scoped.scope {
        ImportScope::Blocks(touched) => validate(
            &RichText::default(),
            &touched,
            EditLevel::Edit,
            &Limits::default(),
        )
        .map_err(CollabError::UpdateRejected),
        ImportScope::Full { before, after } => check_model(&before, after, candidate.level),
    }
}

fn check_model(before: &ModelView, after: ModelView, level: EditLevel) -> Result<(), CollabError> {
    match after {
        ModelView::RichText(after) => {
            let empty = RichText::default();
            let before = match before {
                ModelView::RichText(before) => before,
                _ => &empty,
            };
            validate(before, &after, level, &Limits::default()).map_err(CollabError::UpdateRejected)
        },
        after if level == EditLevel::Comment && after != *before => Err(
            CollabError::UpdateRejected(ModelRejection::BeyondCommentLevel),
        ),
        ModelView::Body(_) => Ok(()),
        #[cfg(feature = "collab-loro")]
        ModelView::Sheet(_) | ModelView::Deck(_) => Ok(()),
    }
}
