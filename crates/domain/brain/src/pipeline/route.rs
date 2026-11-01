//! The route stage.
//!
//! Who a fact is shared with and who owns it. An entered fact keeps the
//! scope its author chose. A document fact is scoped to its folder's
//! project when the document sits in a folder and otherwise to the person
//! it was written for; its owner is that person (the author, or whom an
//! agent wrote on behalf of). A comment fact follows the comment's author
//! the same way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, UserId};
use systemprompt_models::brain::{FactCandidate, FactScope};

use super::normalise::{EventPayload, Normalised};

/// A candidate's scope and owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    pub scope: FactScope,
    pub owner: Option<UserId>,
}

#[must_use]
pub fn route(event: &Normalised, candidate: &FactCandidate) -> Routed {
    let (folder, owner) = match &event.payload {
        EventPayload::Revision(landed) => (
            landed.folder_id.as_ref(),
            landed
                .on_behalf_of
                .clone()
                .unwrap_or_else(|| landed.author.clone()),
        ),
        EventPayload::Comment(resolved) => {
            (resolved.folder_id.as_ref(), resolved.comment_author.clone())
        },
        EventPayload::Entered(_) => (None, event.actor().clone()),
    };
    let scope = candidate
        .scope
        .clone()
        .unwrap_or_else(|| folder_scope(folder, &owner));
    Routed {
        scope,
        owner: Some(owner),
    }
}

fn folder_scope(folder: Option<&DocumentFolderId>, owner: &UserId) -> FactScope {
    folder.map_or_else(
        || FactScope::User(owner.clone()),
        |folder| FactScope::Project(folder.as_str().to_owned()),
    )
}
