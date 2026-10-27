//! Proposal tools: an edit written as a proposal, `documents_proposals`
//! and `documents_review`.
//!
//! A proposed edit acknowledges like any write: `revision_id` is the
//! proposed revision and `written` its lines diffed against the base, plus
//! the `proposal_id` a reviewer acts on. Accepting lands the proposal on
//! the head, applying its ops again when the head has moved; a node that
//! changed in between refuses as `proposal_conflict` with its current text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::service::{ProposeRequest, ReviewRequest};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{DocumentUri, ProposalStatus};
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::acks::remember;
use super::inputs::{ProposalsInput, ReviewAction, ReviewInput};
use super::outputs::{EditOutput, ProposalItem, ProposalsOutput, ReviewOutput};
use super::support::{agent_author, caller, tool_error};
use super::write::{EditCall, written};

pub(super) async fn propose_ops(call: EditCall<'_>) -> Result<(EditOutput, String), McpError> {
    let applied = call.ops.len();
    let proposed = call
        .tools
        .service()
        .propose(
            call.who,
            ProposeRequest {
                document_id: call.document.clone(),
                base_revision: call.base.clone(),
                ops: call.ops.clone(),
                author: agent_author(call.ctx, call.exec_id),
                message: call.message.clone(),
            },
            call.ctx.trace_id(),
        )
        .await
        .map_err(tool_error)?;
    let revision = proposed.revision;
    let written = written(&call, revision.parent_revision_id.as_ref(), &revision.id).await?;
    let summary = format!(
        "proposal {} (revision {}): {applied} ops, {} lines written; a reviewer accepts or \
         rejects it",
        proposed.proposal.id,
        revision.id,
        written.len()
    );
    let verbose = call.verbose;
    let output = EditOutput {
        uri: verbose
            .then(|| DocumentUri::new(call.document.clone()).at_revision(revision.id.clone())),
        parent_revision_id: revision.parent_revision_id.filter(|_| verbose),
        revision_id: revision.id,
        applied,
        merged: false,
        replayed: false,
        proposal_id: Some(proposed.proposal.id),
        written,
    };
    remember(&call, &output).await;
    Ok((output, summary))
}

#[derive(Debug, Clone, Copy)]
pub struct ProposalsHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ProposalsHandler<'_> {
    type Input = ProposalsInput;
    type Output = ProposalsOutput;

    fn tool_name(&self) -> &'static str {
        "documents_proposals"
    }

    fn description(&self) -> &'static str {
        "List a document's proposals (status open, accepted, rejected or superseded)."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: ProposalsInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ProposalsOutput, String), McpError> {
        let who = caller(ctx)?;
        let proposals = self
            .tools
            .service()
            .list_proposals(who, &input.document_id, input.status, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = proposals
            .iter()
            .map(|p| {
                format!(
                    "{} {} by {} ({}): {}",
                    p.id,
                    p.status,
                    p.proposer_user_id,
                    p.actor_kind,
                    p.message.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let proposals = proposals
            .into_iter()
            .map(|p| ProposalItem {
                proposal_id: p.id,
                status: p.status,
                proposer: p.proposer_user_id,
                actor_kind: p.actor_kind,
                base_revision_id: p.base_revision_id,
                proposed_revision_id: p.proposed_revision_id,
                message: p.message,
                created_at: p.created_at,
            })
            .collect();
        Ok((ProposalsOutput { proposals }, summary))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReviewHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ReviewHandler<'_> {
    type Input = ReviewInput;
    type Output = ReviewOutput;

    fn tool_name(&self) -> &'static str {
        "documents_review"
    }

    fn description(&self) -> &'static str {
        "Accept or reject a proposal (needs edit access). See documents_guide topic proposals."
    }

    async fn handle(
        &self,
        input: ReviewInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(ReviewOutput, String), McpError> {
        let who = caller(ctx)?;
        let request = ReviewRequest {
            proposal_id: input.proposal_id,
            note: input.note,
            reviewer: agent_author(ctx, exec_id),
        };
        let service = self.tools.service();
        let output = match input.action {
            ReviewAction::Accept => {
                let accepted = service
                    .accept_proposal(who, request, ctx.trace_id())
                    .await
                    .map_err(tool_error)?;
                ReviewOutput {
                    proposal_id: accepted.proposal.id,
                    status: ProposalStatus::Accepted,
                    revision_id: Some(accepted.revision.id),
                    rebased: accepted.rebased,
                }
            },
            ReviewAction::Reject => {
                let rejected = service
                    .reject_proposal(who, request, ctx.trace_id())
                    .await
                    .map_err(tool_error)?;
                ReviewOutput {
                    proposal_id: rejected.id,
                    status: rejected.status,
                    revision_id: None,
                    rebased: false,
                }
            },
        };
        let summary = match &output.revision_id {
            Some(revision) => format!(
                "proposal {} accepted: head is revision {revision}",
                output.proposal_id
            ),
            None => format!("proposal {} {}", output.proposal_id, output.status),
        };
        Ok((output, summary))
    }
}
