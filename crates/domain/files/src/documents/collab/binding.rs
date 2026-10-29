//! Binding a live session's updates to who sent them: the engine peer ids
//! the server assigns per join, and the actor a checkpoint runs as.
//!
//! A join draws a fresh peer id below 2^53, so a browser can carry it as a
//! JSON number, and records it against the joining user; an update may add
//! operations only of peers its author holds, so no caller can write under
//! another participant's peer and have that participant's later operations
//! dropped as duplicates.
//!
//! The checkpoint actor folds the actor of every accepted update since the
//! last checkpoint: one agent while every such update came from that agent,
//! a human otherwise, and a human when there was none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentEditSessionId, UserId};

use super::entry::CollabEntry;
use super::{CollabError, CollabService};
use crate::documents::agents::{AgentChannel, DocumentActor};

const PEER_BITS: u32 = 53;

impl CollabService {
    pub(super) async fn assign_peer(
        &self,
        session: &DocumentEditSessionId,
        user: &UserId,
    ) -> Result<u64, CollabError> {
        let peer = (uuid::Uuid::new_v4().as_u64_pair().0 >> (u64::BITS - PEER_BITS)).max(1);
        let stored = i64::try_from(peer).unwrap_or(1);
        self.documents
            .repositories()
            .collab
            .assign_peer(session, user, stored)
            .await?;
        Ok(peer)
    }

    pub(super) async fn peers_of(
        &self,
        session: &DocumentEditSessionId,
        user: &UserId,
    ) -> Result<Vec<u64>, CollabError> {
        Ok(self
            .documents
            .repositories()
            .collab
            .list_peers(session, user)
            .await?
            .into_iter()
            .filter_map(|peer| u64::try_from(peer).ok())
            .collect())
    }
}

fn same_actor(left: &DocumentActor, right: &DocumentActor) -> bool {
    left.clone().on(AgentChannel::Direct) == right.clone().on(AgentChannel::Direct)
}

pub(super) fn fold_actor(previous: Option<&DocumentActor>, next: DocumentActor) -> DocumentActor {
    match previous {
        Some(previous) if !(next.is_agent() && same_actor(previous, &next)) => DocumentActor::Human,
        _ => next,
    }
}

pub(super) fn checkpoint_actor(entry: &CollabEntry) -> DocumentActor {
    entry
        .last_actor
        .clone()
        .unwrap_or(DocumentActor::Human)
        .on(AgentChannel::LiveCheckpoint)
}
