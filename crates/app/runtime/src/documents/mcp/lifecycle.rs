//! Lifecycle tools: `documents_trash`, `documents_untrash`,
//! `documents_copy` and `documents_update`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::service::{CopyRequest, UpdateRequest};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::browse::document_item;
use super::inputs::{CopyInput, DocumentInput, UpdateInput};
use super::records::DocumentAck;
use super::support::{agent_author, caller, tool_error};

#[derive(Debug, Clone, Copy)]
pub struct TrashHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for TrashHandler<'_> {
    type Input = DocumentInput;
    type Output = DocumentAck;

    fn tool_name(&self) -> &'static str {
        "documents_trash"
    }

    fn description(&self) -> &'static str {
        "Trash a document."
    }

    async fn handle(
        &self,
        input: DocumentInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(DocumentAck, String), McpError> {
        let who = caller(ctx)?;
        let service = self.tools.service();
        let document = service
            .get(who, &input.document_id, ctx.trace_id())
            .await
            .map_err(tool_error)?
            .document;
        service
            .trash(who, &input.document_id, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = format!("trashed {}", document.name);
        Ok((
            DocumentAck {
                document: document_item(document, false),
                trashed: true,
            },
            summary,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UntrashHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for UntrashHandler<'_> {
    type Input = DocumentInput;
    type Output = DocumentAck;

    fn tool_name(&self) -> &'static str {
        "documents_untrash"
    }

    fn description(&self) -> &'static str {
        "Restore a trashed document."
    }

    async fn handle(
        &self,
        input: DocumentInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(DocumentAck, String), McpError> {
        let who = caller(ctx)?;
        let document = self
            .tools
            .service()
            .untrash(who, &input.document_id, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = format!("untrashed {}", document.name);
        Ok((
            DocumentAck {
                document: document_item(document, false),
                trashed: false,
            },
            summary,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CopyHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for CopyHandler<'_> {
    type Input = CopyInput;
    type Output = DocumentAck;

    fn tool_name(&self) -> &'static str {
        "documents_copy"
    }

    fn description(&self) -> &'static str {
        "Copy a document."
    }

    async fn handle(
        &self,
        input: CopyInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(DocumentAck, String), McpError> {
        let who = caller(ctx)?;
        let (document, _) = self
            .tools
            .service()
            .copy(
                who,
                CopyRequest {
                    source: input.document_id,
                    name: input.name,
                    folder: input.folder_id,
                    author: agent_author(ctx, exec_id),
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let summary = format!("copied to {} ({})", document.name, document.id);
        Ok((
            DocumentAck {
                document: document_item(document, false),
                trashed: false,
            },
            summary,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UpdateHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for UpdateHandler<'_> {
    type Input = UpdateInput;
    type Output = DocumentAck;

    fn tool_name(&self) -> &'static str {
        "documents_update"
    }

    fn description(&self) -> &'static str {
        "Rename or move a document."
    }

    async fn handle(
        &self,
        input: UpdateInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(DocumentAck, String), McpError> {
        let who = caller(ctx)?;
        let document = self
            .tools
            .service()
            .update(
                who,
                &input.document_id,
                UpdateRequest {
                    name: input.name,
                    folder: input.folder_id,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let summary = format!("updated {}", document.name);
        Ok((
            DocumentAck {
                document: document_item(document, false),
                trashed: false,
            },
            summary,
        ))
    }
}

