//! Writing tools: `documents_create`, `documents_edit`,
//! `documents_str_replace` and `documents_set_cells`.
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
use super::inputs::{CreateInput, EditInput, SetCellsInput, StrReplaceInput};
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
        "Create a document from a template (for example blank-docx, blank-xlsx, blank-pptx, \
         blank-html, blank-markdown) in a folder, the root when folder_id is omitted. params \
         fill the template's placeholders; ops apply after instantiation in the same revision. \
         The name gets the format's extension. Returns the written lines to anchor on."
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

macro_rules! edit_handler {
    (
        $handler:ident,
        $input:ty,
        $name:literal,
        $description:literal,
        |$i:ident| $ops:expr,
        $message:expr,
        $verbose:expr
    ) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $handler<'a> {
            pub tools: &'a DocumentsMcpTools,
        }

        impl McpToolHandler for $handler<'_> {
            type Input = $input;
            type Output = EditOutput;

            fn tool_name(&self) -> &'static str {
                $name
            }

            fn description(&self) -> &'static str {
                $description
            }

            async fn handle(
                &self,
                $i: $input,
                ctx: &RequestContext,
                exec_id: &McpExecutionId,
            ) -> Result<(EditOutput, String), McpError> {
                let who = caller(ctx)?;
                let message = $message;
                let verbose = $verbose;
                commit_ops(EditCall {
                    tools: self.tools,
                    who,
                    ctx,
                    exec_id,
                    document: $i.document_id.clone(),
                    base: $i.base_revision.clone(),
                    ops: $ops,
                    message,
                    verbose,
                })
                .await
            }
        }
    };
}

edit_handler!(
    EditHandler,
    EditInput,
    "documents_edit",
    "Apply edit ops to a document as one revision against base_revision. Anchored ops \
     (replace_text, delete) quote the node id and hash from documents_read as node and \
     expected_hash; a stale hash is refused with the node's current text. Every target \
     resolves against the document as it stood before the call, and ops apply in order: two \
     insert_after on one node land in op order. insert_after kinds differ per format; a refusal \
     names accepted_kinds. All ops apply or none; message is kept in the history.",
    |input| input.ops.clone(),
    input.message.clone(),
    input.verbose
);

edit_handler!(
    StrReplaceHandler,
    StrReplaceInput,
    "documents_str_replace",
    "Replace old_str with new_str in the document text as one revision against base_revision. \
     old_str must occur exactly once; the error reports how many places it matched.",
    |input| vec![EditOp::StrReplace {
        old_str: input.old_str.clone(),
        new_str: input.new_str.clone(),
    }],
    input.message.clone(),
    false
);

edit_handler!(
    SetCellsHandler,
    SetCellsInput,
    "documents_set_cells",
    "Write a rectangle of spreadsheet values (rows of cells) into sheet at an A1 range such as \
     B7 or A1:C9, as one revision against base_revision.",
    |input| vec![EditOp::SetCells {
        sheet: input.sheet.clone(),
        range: input.range.clone(),
        values: input.values.clone(),
        expected_hash: None,
    }],
    input.message.clone(),
    false
);
