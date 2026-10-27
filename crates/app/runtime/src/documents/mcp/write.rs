//! Writing tools: `documents_create`, `documents_edit`,
//! `documents_str_replace` and `documents_set_cells`.
//!
//! Every edit is a commit against the caller's `base_revision`, authored by
//! an agent with the MCP execution as its origin. A base that is no longer
//! the head is answered with the head's id, author and time so the agent can
//! re-read and resend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommitOutcome, CommitRequest, CreateRequest};
use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::ApiPaths;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentUri, EditOp};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_provider_contracts::documents::TemplateParams;

use super::DocumentsMcpTools;
use super::inputs::{CreateInput, EditInput, SetCellsInput, StrReplaceInput};
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
}

fn touched(op: &EditOp) -> Option<DocumentNodeId> {
    match op {
        EditOp::SetCells { sheet, range, .. } => {
            DocumentNodeId::try_new(format!("sheet[{sheet}]/{range}")).ok()
        },
        other => other.touches().cloned(),
    }
}

fn reread_hint(ops: &[EditOp], revision: &DocumentRevisionId) -> Option<String> {
    let structural = ops.iter().any(|op| {
        matches!(
            op,
            EditOp::InsertAfter { .. } | EditOp::Delete { .. } | EditOp::InsertSlide { .. }
        )
    });
    let rewritten = ops.iter().any(|op| {
        matches!(
            op,
            EditOp::StrReplace { .. } | EditOp::SetHtml { .. } | EditOp::SetText { .. }
        )
    });
    if structural {
        Some(format!(
            "nodes were inserted or deleted, so positional node ids after them may have shifted; \
             read the outline of revision {revision} before the next anchored edit"
        ))
    } else if rewritten {
        Some(format!(
            "text was rewritten; read revision {revision} again for current node hashes"
        ))
    } else {
        None
    }
}

async fn commit_ops(call: EditCall<'_>) -> Result<(EditOutput, String), McpError> {
    let applied = call.ops.len();
    let ops = call.ops.clone();
    let outcome = call
        .tools
        .service()
        .commit(
            call.who,
            CommitRequest {
                document_id: call.document.clone(),
                base_revision: Some(call.base.clone()),
                body: CommitBody::Ops(call.ops),
                author: agent_author(call.exec_id),
                message: call.message,
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
    let at = DocumentUri::new(call.document).at_revision(revision.id.clone());
    let summary = format!("revision {} with {applied} edits", revision.id);
    Ok((
        EditOutput {
            uri: at.clone(),
            changed_nodes: ops
                .iter()
                .filter_map(touched)
                .map(|node| at.clone().at_node(node))
                .collect(),
            reread_hint: reread_hint(&ops, &revision.id),
            revision_id: revision.id,
            parent_revision_id: revision.parent_revision_id,
            applied,
            merged,
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
         fill the template's placeholders. The name gets the format's extension."
    }

    async fn handle(
        &self,
        input: CreateInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(CreateOutput, String), McpError> {
        let who = caller(ctx)?;
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
                    author: agent_author(exec_id),
                    message: input.message,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let preview_url = format!(
            "{}{}/{}/preview",
            self.tools.api_external_url().trim_end_matches('/'),
            ApiPaths::DOCUMENTS,
            document.id
        );
        let summary = format!("created {} at revision {}", document.name, revision.id);
        Ok((
            CreateOutput {
                uri: DocumentUri::new(document.id.clone()).at_revision(revision.id.clone()),
                document_id: document.id,
                revision_id: revision.id,
                format: document.format,
                preview_url,
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
        $message:expr
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
                commit_ops(EditCall {
                    tools: self.tools,
                    who,
                    ctx,
                    exec_id,
                    document: $i.document_id.clone(),
                    base: $i.base_revision.clone(),
                    ops: $ops,
                    message,
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
    input.message.clone()
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
    input.message.clone()
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
    input.message.clone()
);
