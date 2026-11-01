//! One engine peer per live session and actor for op batches.
//!
//! The first batch an actor writes into a session is assigned a peer
//! (`document_collab_peers`); later batches of the same actor in the same
//! session reuse it, so a long agent run adds one peer row, not one per
//! call. The map is per replica: another replica assigns its own peer for
//! the same actor, which is still bounded by the replica count. It is
//! cleared whole when it grows past its bound, which only costs fresh peers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use systemprompt_identifiers::{DocumentEditSessionId, UserId};

use crate::documents::agents::DocumentActor;
use crate::documents::collab::{CollabError, CollabService};

const MAX_PEERS: usize = 4096;

type PeerKey = (DocumentEditSessionId, UserId, String);

#[derive(Debug, Default)]
pub(crate) struct AgentPeers(Mutex<HashMap<PeerKey, u64>>);

impl CollabService {
    pub(super) async fn actor_peer(
        &self,
        session: &DocumentEditSessionId,
        user: &UserId,
        actor: &DocumentActor,
    ) -> Result<u64, CollabError> {
        let key = (session.clone(), user.clone(), format!("{actor:?}"));
        if let Some(peer) = self
            .agent_peers
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Ok(*peer);
        }
        let peer = self.assign_peer(session, user).await?;
        let mut peers = self
            .agent_peers
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if peers.len() >= MAX_PEERS {
            peers.clear();
        }
        Ok(*peers.entry(key).or_insert(peer))
    }
}
