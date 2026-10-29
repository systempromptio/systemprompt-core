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
use crate::documents::agents::{AgentActor, AgentCapability, AgentRateLimiter, AgentRefusal, Reservation};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::revision::CommitBody;

const MIN_BROAD_REWRITE: usize = 3;

impl DocumentService {
    pub(super) async fn agent_write_rules(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        body: &CommitBody,
    ) -> Result<(), DocumentsError> {
        if !self.access.actor().is_agent() {
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
            CommitBody::Ops(_) => None,
        };
        if let Some(capability) = capability {
            return self
                .access
                .require_agent_capability(who, document, capability)
                .await;
        }
        let CommitBody::Ops(ops) = body else {
            return Ok(());
        };
        self.broad_rewrite_rule(who, document, ops).await
    }

    /// An agent that has rewritten, within the hour, as many of a document's
    /// blocks as replacing it needs `BroadRewrite`; the count is recorded only
    /// for a write the rule lets through, so a refused attempt never counts.
    async fn broad_rewrite_rule(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        ops: &[EditOp],
    ) -> Result<(), DocumentsError> {
        let Some((touched, agent)) = self.rewritten_nodes(ops) else {
            return Ok(());
        };
        let now = Instant::now();
        let reach = self
            .agent_rewrites
            .would_reach(&document.id, &agent.name, &touched, now);
        if reach >= MIN_BROAD_REWRITE {
            let head = self.resolve_revision(document, None).await?;
            let blocks = self.projected(document, &head).await?.ids.nodes().count();
            if reach.saturating_mul(2) > blocks {
                let allowed = self
                    .access
                    .require_agent_capability(who, document, AgentCapability::BroadRewrite)
                    .await;
                if let Err(DocumentsError::Agent(AgentRefusal::HardRule { .. })) = allowed {
                    return Err(AgentRefusal::BroadRewrite {
                        agent: agent.name,
                        rewritten: reach,
                        blocks,
                    }
                    .into());
                }
                allowed?;
            }
        }
        self.agent_rewrites
            .touch(&document.id, &agent.name, &touched, now);
        Ok(())
    }

    pub(super) fn reserve_agent_edits(
        &self,
        who: &AuthenticatedUser,
        ops: usize,
    ) -> Result<EditReservation<'_>, DocumentsError> {
        let reservation = match self.access.actor().as_agent() {
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
    fn rewritten_nodes(&self, ops: &[EditOp]) -> Option<(HashSet<&DocumentNodeId>, AgentActor)> {
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
            .filter_map(|op| op.touches().or_else(|| op.rich_guard().and_then(|(node, _)| node.id())))
            .collect();
        if touched.is_empty() {
            return None;
        }
        let agent = self.access.actor().as_agent().cloned()?;
        Some((touched, agent))
    }
}

pub(super) struct EditReservation<'a> {
    limiter: &'a AgentRateLimiter,
    reservation: Option<Reservation>,
}

impl EditReservation<'_> {
    pub(super) fn keep(mut self) {
        self.reservation = None;
    }

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

pub(super) const fn body_ops(body: &CommitBody) -> usize {
    match body {
        CommitBody::Ops(ops) => ops.len(),
        CommitBody::Bytes(_) => 1,
    }
}
