//! The write rules an agent caller meets on top of its cap: no restore of
//! an earlier revision and no whole-content replacement unless the agent
//! access policy allows it, and a per-user ceiling on agent edits per hour
//! (`files.documents.limits.agentEditsPerHour`). A person passes untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;

use super::DocumentService;
use crate::documents::agents::{AgentCapability, current_actor};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::revision::CommitBody;

impl DocumentService {
    pub(super) async fn agent_write_rules(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        body: &CommitBody,
    ) -> Result<(), DocumentsError> {
        if !current_actor().is_agent() {
            return Ok(());
        }
        let capability = match body {
            CommitBody::Bytes(_) => Some(AgentCapability::ReplaceContent),
            CommitBody::Ops(ops) if ops.iter().any(|op| matches!(op, EditOp::Restore { .. })) => {
                Some(AgentCapability::RestoreRevision)
            },
            CommitBody::Ops(_) => None,
        };
        match capability {
            Some(capability) => {
                self.access
                    .require_agent_capability(who, document, capability)
                    .await
            },
            None => Ok(()),
        }
    }

    /// Counts `ops` agent edits for the calling user, or refuses them as
    /// `agent_rate_limited`.
    pub(super) fn admit_agent_edits(
        &self,
        who: &AuthenticatedUser,
        ops: usize,
    ) -> Result<(), DocumentsError> {
        if !current_actor().is_agent() {
            return Ok(());
        }
        let ops = u64::try_from(ops).unwrap_or(u64::MAX);
        Ok(self.agent_edits.admit(&who.id, ops, Instant::now())?)
    }
}

pub(super) fn body_ops(body: &CommitBody) -> usize {
    match body {
        CommitBody::Ops(ops) => ops.len(),
        CommitBody::Bytes(_) => 1,
    }
}
