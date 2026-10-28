//! Open proposals on the user's documents, refreshed once per sync cycle
//! and kept in the Drive state so `drive status` reads them offline.
//!
//! A failed fetch keeps the last list and is logged; proposals never fail a
//! sync cycle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Cycle;
use super::state::{DriveState, OpenProposal};

pub async fn refresh(cycle: Cycle<'_>, state: &mut DriveState) -> bool {
    let fetched = match cycle.client.proposals(cycle.bearer).await {
        Ok(fetched) => fetched,
        Err(error) => {
            tracing::warn!(%error, "drive proposals not refreshed; the last list stands");
            return false;
        },
    };
    let proposals: Vec<OpenProposal> = fetched
        .proposals
        .into_iter()
        .map(|p| OpenProposal {
            path: state.path_of(&p.document_id).map(str::to_owned),
            proposal_id: p.proposal_id,
            document_id: p.document_id,
            proposer_id: p.proposer_id,
            message: p.message,
            created_at: p.created_at,
        })
        .collect();
    if proposals == state.proposals {
        return false;
    }
    state.proposals = proposals;
    true
}
