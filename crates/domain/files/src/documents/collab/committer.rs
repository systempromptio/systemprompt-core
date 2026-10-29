//! Who a checkpoint commits as, read from the shared update log.
//!
//! Claiming a checkpoint lease reads the updates the session's last
//! checkpoint does not cover. Their recorded actors fold into the checkpoint
//! actor, so every replica checkpoints under the same actor whichever
//! replica admitted the updates. The checkpoint commits as the most recent
//! author whose update was admitted at edit level. When only comment-level
//! authors wrote since the checkpoint, comment access may propose but not
//! commit: a settle keeps the body as a proposal by the most recent of them
//! and the periodic checkpoint leaves the body to that settle. When nobody
//! wrote there is nothing to commit. A user who never wrote to the session
//! never commits it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::AuthenticatedUser;

use super::binding::fold_actor;
use crate::documents::agents::DocumentActor;
use crate::documents::model::EditLevel;
use crate::documents::repository::LoggedAuthor;

#[derive(Debug, Clone)]
pub(super) enum Committer {
    Editor(AuthenticatedUser),
    Commenter(AuthenticatedUser),
    Nobody,
}

impl Committer {
    pub(super) fn from_log(authors: &[LoggedAuthor], known: &[&AuthenticatedUser]) -> Self {
        let latest = |level: EditLevel| {
            authors
                .iter()
                .rev()
                .find(|author| author.level.unwrap_or(EditLevel::Edit) == level)
        };
        if let Some(author) = latest(EditLevel::Edit) {
            return Self::Editor(user_for(&author.user, known));
        }
        if let Some(author) = latest(EditLevel::Comment) {
            return Self::Commenter(user_for(&author.user, known));
        }
        Self::Nobody
    }
}

pub(super) fn fold_logged(authors: &[LoggedAuthor]) -> Option<DocumentActor> {
    authors.iter().fold(None, |folded, author| {
        Some(fold_actor(
            folded.as_ref(),
            author.actor.clone().unwrap_or(DocumentActor::Human),
        ))
    })
}

fn user_for(id: &UserId, known: &[&AuthenticatedUser]) -> AuthenticatedUser {
    known.iter().find(|user| user.id == *id).map_or_else(
        || AuthenticatedUser::new(id.clone(), String::new(), String::new(), Vec::new()),
        |user| (*user).clone(),
    )
}
