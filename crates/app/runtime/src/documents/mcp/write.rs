//! Writing tools: `documents_create` and `documents_edit`.
//!
//! Every edit is a commit against the caller's `base_revision`, authored by
//! an agent with the MCP execution as its origin. A base that is no longer
//! the head is refused as `stale_base` with the head's id, author and time.
//! Every write answers with the lines it wrote, diffed against the parent
//! revision, so the next edit anchors on the ack.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommitOutcome, CommitRequest, CreateRequest};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, McpExecutionId};
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
    base: DocumentRevisionId,
    ops: Vec<EditOp>,
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

async fn commit_ops(call: EditCall<'_>) -> Result<(EditOutput, String), McpError> {
    let applied = call.ops.len();
    let outcome = call
        .tools
        .service()
        .commit(
            call.who,
            CommitRequest {
                document_id: call.document.clone(),
                base_revision: Some(call.base.clone()),
                body: CommitBody::Ops(call.ops.clone()),
                author: agent_author(call.exec_id),
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
    Ok((
        EditOutput {
            uri: verbose
                .then(|| DocumentUri::new(call.document.clone()).at_revision(revision.id.clone())),
            parent_revision_id: revision.parent_revision_id.filter(|_| verbose),
            revision_id: revision.id,
            applied,
            merged,
            written,
        },
        summary,
    ))
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
                    author: agent_author(exec_id),
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
         set_text, set_html) as one revision against base_revision; returns the written lines. \
         See documents_guide topic editing."
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
            message: input.message,
            verbose: input.verbose,
        })
        .await
    }
}
