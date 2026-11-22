//! The editing tool: `documents_edit`.
//!
//! Every edit is a commit applied to the head, authored by an agent with the
//! MCP execution as its origin and the request's session, context, trace
//! and task as its attribution. Each anchored op guards only the node it
//! touches: a node that changed at the head is refused as `guard_mismatch`
//! with its current text, and an edit that landed while the head moved past
//! `base_revision` answers `merged`. A `restore` whose base is not the head
//! is refused as `stale_base`. An `op_id` makes the call idempotent: the
//! commit stores its acknowledgement in the same transaction, and a repeat
//! of the same request returns it with `replayed`. Every write answers with
//! the lines it wrote, diffed against the parent revision, so the next edit
//! anchors on the ack. An edit whose mode resolves to
//! `propose` is written as a proposal instead (see `proposals`), and when
//! the caller did not ask for one (an agent capped at `suggest`) the summary
//! says it was converted. An edit on a document with an open live session
//! is written into that session instead of onto the head (see `live_edit`).
//!
//! In place of `ops`, `content_blob_ref` names a blob the caller uploaded
//! with `documents_upload` as the document's whole new content, written on
//! the `base_revision` the caller read (required with it): the bytes
//! pass the document format's checks and land byte for byte as one revision
//! (or one proposal), so an HTML page keeps its scripts. A landed whole-file
//! write by an agent needs the access policy's `replace_content`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    Author, CommentPolicy, CommitOutcome, CommitRequest, EditModeRequest, ExternalWrite,
};
use systemprompt_identifiers::{
    ContentHash, DocumentId, DocumentOpId, DocumentRevisionId, McpExecutionId,
};
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{EditMode, EditOp};
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::agent_presence::announce_agent;
use super::import::owned_blob;
use super::inputs::EditInput;
use super::live_edit::edit_live;
use super::media::{image_refs, resolve_media};
use super::proposals::{proposal_ack, propose_ops};
use super::refusal::refusal;
use super::support::{agent_author, caller, stale_base, tool_error};
use crate::documents::refusal::RefusalCode;
use crate::documents::views::{EditOutput, edit_ack};

const CONVERTED: &str = "Written as a proposal, not landed: agents may only suggest on this \
                         document (its agent cap is suggest, or its folder's agent edit policy \
                         is propose), or you hold comment but not edit";

pub(super) struct EditCall<'a> {
    pub(super) tools: &'a DocumentsMcpTools,
    pub(super) who: &'a AuthenticatedUser,
    pub(super) ctx: &'a RequestContext,
    pub(super) author: Author,
    pub(super) document: DocumentId,
    pub(super) base: Option<DocumentRevisionId>,
    pub(super) ops: Vec<EditOp>,
    pub(super) mode: Option<EditMode>,
    pub(super) op_id: Option<DocumentOpId>,
    pub(super) message: Option<String>,
    pub(super) comments: CommentPolicy,
    pub(super) verbose: bool,
}

async fn commit_ops(call: EditCall<'_>) -> Result<(EditOutput, String), McpError> {
    let mode = call
        .tools
        .service()
        .edit_mode(
            call.who,
            EditModeRequest {
                document: &call.document,
                requested: call.mode,
            },
            call.ctx.trace_id(),
        )
        .await
        .map_err(tool_error)?;
    if mode == EditMode::Propose {
        let asked = call.mode;
        let (output, summary) = propose_ops(call).await?;
        if asked == Some(EditMode::Propose) {
            return Ok((output, summary));
        }
        return Ok((output, format!("{summary}. {CONVERTED}")));
    }
    let refs = image_refs(None, &call.ops);
    let media = resolve_media(call.tools, call.ctx, refs, Some(&call.document)).await?;
    let request = CommitRequest {
        document_id: call.document.clone(),
        base_revision: call.base.clone(),
        body: CommitBody::Ops(call.ops.clone()),
        author: call.author.clone(),
        message: call.message.clone(),
        lock_holder: None,
        comments: call.comments.clone(),
        op_id: call.op_id.clone(),
        media,
    };
    if let Some(output) = edit_live(&call, &request).await? {
        announce_agent(&call, &output).await;
        let summary = format!("wrote {} live", output.written.len());
        return Ok((output, summary));
    }
    let outcome = call
        .tools
        .service()
        .commit(call.who, request, call.ctx.trace_id())
        .await
        .map_err(tool_error)?;
    landed_ack(&call, outcome).await
}

async fn landed_ack(
    call: &EditCall<'_>,
    outcome: CommitOutcome,
) -> Result<(EditOutput, String), McpError> {
    let landed = match outcome {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => *landed,
        CommitOutcome::Conflict { head, base } => {
            return Err(stale_base(&call.document, &head, base.as_ref()));
        },
    };
    let output = edit_ack(&call.document, landed, call.verbose);
    if !output.replayed {
        announce_agent(call, &output).await;
    }
    let summary = if output.replayed {
        format!(
            "op {} replayed",
            call.op_id.as_ref().map_or("", |op| op.as_str())
        )
    } else {
        format!("wrote {}", output.written.len())
    };
    Ok((output, summary))
}

async fn replace_body(
    call: EditCall<'_>,
    blob: &ContentHash,
) -> Result<(EditOutput, String), McpError> {
    let service = call.tools.service();
    let trace = call.ctx.trace_id();
    let mode = service
        .edit_mode(
            call.who,
            EditModeRequest {
                document: &call.document,
                requested: call.mode,
            },
            trace,
        )
        .await
        .map_err(tool_error)?;
    let format = service
        .get(call.who, &call.document, trace)
        .await
        .map_err(tool_error)?
        .document
        .format;
    let (bytes, _) = owned_blob(
        call.tools,
        call.ctx.user_id(),
        blob,
        format,
        service.config().max_upload_bytes,
    )
    .await?;
    let request = CommitRequest {
        document_id: call.document.clone(),
        base_revision: call.base.clone(),
        body: CommitBody::Bytes(bytes),
        author: call.author.clone(),
        message: call.message.clone(),
        lock_holder: None,
        comments: call.comments.clone(),
        op_id: call.op_id.clone(),
        media: Media::new(),
    };
    let written = if mode == EditMode::Propose {
        ExternalWrite::Proposed(Box::new(
            service
                .propose_write(call.who, request, trace)
                .await
                .map_err(tool_error)?,
        ))
    } else {
        service
            .write_external(call.who, request, trace)
            .await
            .map_err(tool_error)?
    };
    match written {
        ExternalWrite::Landed(outcome) => landed_ack(&call, outcome).await,
        ExternalWrite::Proposed(proposed) => {
            let (output, summary) = proposal_ack(&call, *proposed, 1).await?;
            if call.mode == Some(EditMode::Propose) {
                return Ok((output, summary));
            }
            Ok((output, format!("{summary}. {CONVERTED}")))
        },
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EditHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for EditHandler<'_> {
    type Input = EditInput;
    type Output = EditOutput;

    fn tool_name(&self) -> &'static str {
        "documents_edit"
    }

    fn description(&self) -> &'static str {
        "Apply ops or replace the whole file from a content_blob_ref, or propose either. \
         Guide: editing."
    }

    async fn handle(
        &self,
        input: EditInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(EditOutput, String), McpError> {
        let who = caller(ctx)?;
        let blob = match (input.ops.is_empty(), input.content_blob_ref) {
            (true, None) => {
                return Err(refusal(
                    RefusalCode::InvalidParams,
                    "documents_edit needs ops, or content_blob_ref to replace the whole file",
                ));
            },
            (false, Some(_)) => {
                return Err(refusal(
                    RefusalCode::InvalidParams,
                    "ops and content_blob_ref are exclusive: send ops to edit blocks, or \
                     content_blob_ref alone to replace the whole file",
                ));
            },
            (true, Some(_)) if input.base_revision.is_none() => {
                return Err(refusal(
                    RefusalCode::InvalidParams,
                    "content_blob_ref replaces the revision you read: pass it as base_revision",
                ));
            },
            (_, blob) => blob,
        };
        let author = agent_author(self.tools, ctx, exec_id, input.context_id.as_ref()).await?;
        let call = EditCall {
            tools: self.tools,
            who,
            ctx,
            author,
            document: input.document_id,
            base: input.base_revision,
            ops: input.ops,
            mode: input.mode,
            op_id: input.op_id,
            message: input.message,
            comments: CommentPolicy {
                allow_detach: input.allow_detach,
                answering: input.answering,
            },
            verbose: input.verbose,
        };
        match blob {
            Some(blob) => replace_body(call, &blob).await,
            None => commit_ops(call).await,
        }
    }
}
