//! Writing tools: `documents_create` and `documents_edit`.
//!
//! Every edit is a commit applied to the head, authored by an agent with the
//! MCP execution as its origin and the request's session, context, trace
//! and task as its attribution. Each anchored op guards only the node it
//! touches: a node that changed at the head is refused as `guard_mismatch`
//! with its current text, and an edit that landed while the head moved past
//! `base_revision` answers `merged`. A `restore` whose base is not the head
//! is refused as `stale_base`. An `op_id` makes the call idempotent: a
//! repeat returns the stored acknowledgement with `replayed`. Every write
//! answers with the lines it wrote, diffed against the parent revision, so
//! the next edit anchors on the ack.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::StoredWriteAck;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommitOutcome, CommitRequest, CreateRequest};
use systemprompt_identifiers::{DocumentId, DocumentOpId, DocumentRevisionId, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentUri, EditOp};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_provider_contracts::documents::TemplateParams;

use super::DocumentsMcpTools;
use super::inputs::{CreateInput, EditInput};
use super::lines::written_lines;
use super::outputs::{CreateOutput, EditOutput};
use super::support::{agent_author, caller, stale_base, tool_error};

struct EditCall<'a> {
    tools: &'a DocumentsMcpTools,
    who: &'a AuthenticatedUser,
    ctx: &'a RequestContext,
    exec_id: &'a McpExecutionId,
    document: DocumentId,
    base: Option<DocumentRevisionId>,
    ops: Vec<EditOp>,
    op_id: Option<DocumentOpId>,
    message: Option<String>,
    verbose: bool,
}

async fn written(
    call: &EditCall<'_>,
    parent: Option<&DocumentRevisionId>,
    revision: &DocumentRevisionId,
) -> Result<Vec<String>, McpError> {
    let service = call.tools.service();
    let trace = call.ctx.trace_id();
    let (_, head) = service
        .project(call.who, &call.document, Some(revision), trace)
        .await
        .map_err(tool_error)?;
    let base = match parent {
        Some(parent) => Some(
            service
                .project(call.who, &call.document, Some(parent), trace)
                .await
                .map_err(tool_error)?
                .1,
        ),
        None => None,
    };
    Ok(written_lines(base.as_ref(), &head))
}

async fn replay(call: &EditCall<'_>) -> Result<Option<(EditOutput, String)>, McpError> {
    let Some(op_id) = call.op_id.as_ref() else {
        return Ok(None);
    };
    let Some(stored) = call
        .tools
        .service()
        .find_write_ack(call.who, &call.document, op_id, call.ctx.trace_id())
        .await
        .map_err(tool_error)?
    else {
        return Ok(None);
    };
    let mut output: EditOutput = serde_json::from_value(stored.ack).map_err(|error| {
        tracing::error!(error = %error, op_id = %op_id, "Stored write ack does not decode");
        McpError::internal_error("the stored acknowledgement could not be read", None)
    })?;
    output.replayed = true;
    let summary = format!(
        "revision {}: op {op_id} already applied",
        output.revision_id
    );
    Ok(Some((output, summary)))
}

async fn remember(call: &EditCall<'_>, output: &EditOutput) {
    let Some(op_id) = call.op_id.as_ref() else {
        return;
    };
    let ack = match serde_json::to_value(output) {
        Ok(ack) => ack,
        Err(error) => {
            tracing::warn!(error = %error, op_id = %op_id, "Write ack not stored");
            return;
        },
    };
    let stored = StoredWriteAck {
        revision_id: output.revision_id.clone(),
        ack,
    };
    if let Err(error) = call
        .tools
        .service()
        .record_write_ack(&call.document, op_id, &stored)
        .await
    {
        tracing::warn!(error = %error, op_id = %op_id, "Write ack not stored");
    }
}

async fn commit_ops(call: EditCall<'_>) -> Result<(EditOutput, String), McpError> {
    if let Some(replayed) = replay(&call).await? {
        return Ok(replayed);
    }
    let applied = call.ops.len();
    let outcome = call
        .tools
        .service()
        .commit(
            call.who,
            CommitRequest {
                document_id: call.document.clone(),
                base_revision: call.base.clone(),
                body: CommitBody::Ops(call.ops.clone()),
                author: agent_author(call.ctx, call.exec_id),
                message: call.message.clone(),
                lock_holder_id: None,
            },
            call.ctx.trace_id(),
        )
        .await
        .map_err(tool_error)?;
    let (revision, merged) = match outcome {
        CommitOutcome::Committed { revision, .. } => (revision, false),
        CommitOutcome::Merged { revision, .. } => (revision, true),
        CommitOutcome::Conflict { head, base } => {
            return Err(stale_base(&call.document, &head, base.as_ref()));
        },
    };
    let written = written(&call, revision.parent_revision_id.as_ref(), &revision.id).await?;
    let summary = format!(
        "revision {}: {applied} ops, {} lines written",
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
        merged,
        replayed: false,
        written,
    };
    remember(&call, &output).await;
    Ok((output, summary))
}

#[derive(Debug, Clone, Copy)]
pub struct CreateHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for CreateHandler<'_> {
    type Input = CreateInput;
    type Output = CreateOutput;

    fn tool_name(&self) -> &'static str {
        "documents_create"
    }

    fn description(&self) -> &'static str {
        "Create a document from a template (blank-docx, blank-xlsx, blank-pptx, blank-html, \
         blank-markdown, ...), optionally applying ops in the same revision; returns the written \
         lines. See documents_guide topic editing."
    }

    async fn handle(
        &self,
        input: CreateInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(CreateOutput, String), McpError> {
        let who = caller(ctx)?;
        let applied = input.ops.len();
        let (document, revision) = self
            .tools
            .service()
            .create_from_template(
                who,
                CreateRequest {
                    template_id: input.template_id,
                    folder: input.folder_id,
                    name: input.name,
                    params: TemplateParams {
                        values: input.params,
                    },
                    ops: input.ops,
                    author: agent_author(ctx, exec_id),
                    message: input.message,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let (_, projection) = self
            .tools
            .service()
            .project(who, &document.id, Some(&revision.id), ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let written = written_lines(None, &projection);
        let summary = format!(
            "created {} at revision {}: {applied} ops, {} lines written",
            document.name,
            revision.id,
            written.len()
        );
        Ok((
            CreateOutput {
                document_id: document.id,
                revision_id: revision.id,
                format: document.format,
                name: document.name,
                written,
            },
            summary,
        ))
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
        "Apply ops (replace_text, insert_after, delete, str_replace, set_cells, insert_slide, \
         set_text, set_html, append_markdown, restore, insert_from_revision) as one revision on \
         the head; returns the written lines. Anchored ops name node by stable id with \
         expected_hash, or node: {find, nth?, within?}; only a touched node that changed \
         refuses (the head edit wins). op_id makes a retry safe. See documents_guide topic \
         editing."
    }

    async fn handle(
        &self,
        input: EditInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(EditOutput, String), McpError> {
        let who = caller(ctx)?;
        commit_ops(EditCall {
            tools: self.tools,
            who,
            ctx,
            exec_id,
            document: input.document_id,
            base: input.base_revision,
            ops: input.ops,
            op_id: input.op_id,
            message: input.message,
            verbose: input.verbose,
        })
        .await
    }
}
