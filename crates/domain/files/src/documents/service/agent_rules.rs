//! The write rules an agent caller meets on top of its cap: no restore of
//! an earlier revision and no whole-content replacement (a whole-file body,
//! `set_text` or `set_html`, or ops rewriting or deleting more than half the
//! head's nodes) unless the agent
//! access policy allows it, and a ceiling on agent edits per hour per user
//! and agent
//! (`files.documents.limits.agentEditsPerHour`). A person passes untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::time::Instant;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;

use super::{CommitOutcome, DocumentService};
use crate::documents::agents::{AgentCapability, AgentRateLimiter, Reservation, current_actor};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::revision::CommitBody;

/// Fewer touched nodes than this never count as a broad rewrite.
const MIN_BROAD_REWRITE: usize = 3;

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
        let restores = |op: &EditOp| matches!(op, EditOp::Restore { .. });
        let replaces = |op: &EditOp| matches!(op, EditOp::SetText { .. } | EditOp::SetHtml { .. });
        let capability = match body {
            CommitBody::Bytes(_) => Some(AgentCapability::ReplaceContent),
            CommitBody::Ops(ops) if ops.iter().any(restores) => {
                Some(AgentCapability::RestoreRevision)
            },
            CommitBody::Ops(ops) if ops.iter().any(replaces) => {
                Some(AgentCapability::ReplaceContent)
            },
            CommitBody::Ops(ops) if self.rewrites_most(document, ops).await? => {
                Some(AgentCapability::ReplaceContent)
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

    /// Reserves a batch of `ops` agent edits against the hourly ceilings
    /// for the calling user and agent, or refuses it (`agent_rate_limited`).
    /// The reservation is handed back on drop unless it is kept, so any
    /// early return refunds it.
    pub(super) fn reserve_agent_edits(
        &self,
        who: &AuthenticatedUser,
        ops: usize,
    ) -> Result<EditReservation<'_>, DocumentsError> {
        let reservation = match current_actor().as_agent() {
            Some(agent) => Some(self.agent_edits.reserve(
                &who.id,
                &agent.name,
                u64::try_from(ops).unwrap_or(u64::MAX),
                Instant::now(),
            )?),
            None => None,
        };
        Ok(EditReservation {
            limiter: &self.agent_edits,
            reservation,
        })
    }
}

impl DocumentService {
    /// Whether this agent's rewrites of the document within the hour, these
    /// `ops` included, touch more than half the head's nodes: an agent
    /// rewriting most of a document node by node, in one commit or several,
    /// is replacing it.
    async fn rewrites_most(
        &self,
        document: &Document,
        ops: &[EditOp],
    ) -> Result<bool, DocumentsError> {
        let rewrites = |op: &&EditOp| {
            matches!(
                op,
                EditOp::ReplaceText { .. }
                    | EditOp::Delete { .. }
                    | EditOp::SetMarks(_)
                    | EditOp::SetBlockType(_)
            )
        };
        let touched: HashSet<&DocumentNodeId> = ops
            .iter()
            .filter(rewrites)
            .filter_map(EditOp::touches)
            .collect();
        if touched.is_empty() {
            return Ok(false);
        }
        let Some(agent) = current_actor().as_agent().cloned() else {
            return Ok(false);
        };
        let cumulative =
            self.agent_rewrites
                .touch(&document.id, &agent.name, &touched, Instant::now());
        if cumulative < MIN_BROAD_REWRITE {
            return Ok(false);
        }
        let head = self.resolve_revision(document, None).await?;
        let nodes = self.projected(document, &head).await?.ids.nodes().count();
        Ok(cumulative.saturating_mul(2) > nodes)
    }
}

/// A reserved batch of agent edits, refunded when dropped unless kept.
pub(super) struct EditReservation<'a> {
    limiter: &'a AgentRateLimiter,
    reservation: Option<Reservation>,
}

impl EditReservation<'_> {
    /// The batch landed: keep it counted.
    pub(super) fn keep(mut self) {
        self.reservation = None;
    }

    /// Keeps the batch when `outcome` landed (`Committed` or `Merged`).
    pub(super) fn keep_if_landed(self, outcome: &Result<CommitOutcome, DocumentsError>) {
        if matches!(
            outcome,
            Ok(CommitOutcome::Committed(_) | CommitOutcome::Merged(_))
        ) {
            self.keep();
        }
    }
}

impl Drop for EditReservation<'_> {
    fn drop(&mut self) {
        if let Some(reservation) = self.reservation.take() {
            self.limiter.refund(&reservation);
        }
    }
}

pub(super) fn body_ops(body: &CommitBody) -> usize {
    match body {
        CommitBody::Ops(ops) => ops.len(),
        CommitBody::Bytes(_) => 1,
    }
}
