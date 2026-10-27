//! Comment threads anchor to a stable node id and an optional quote inside
//! it, so a thread survives edits elsewhere in the document and is detached,
//! not lost, when its words go.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::CommentAnchor;
use systemprompt_files::documents::service::{CommentRequest, ResolveCommentRequest};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{CommentAction, CommentInput};
use super::records::{CommentItem, CommentOutput};
use super::refusal::{RefusalCode, refusal};
use super::support::{caller, tool_error};

#[derive(Debug, Clone, Copy)]
pub struct CommentHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl CommentHandler<'_> {
    async fn create(
        &self,
        who: &AuthenticatedUser,
        input: CommentInput,
        ctx: &RequestContext,
    ) -> Result<CommentOutput, McpError> {
        let body = input
            .body
            .filter(|b| !b.trim().is_empty())
            .ok_or_else(|| refusal(RefusalCode::InvalidParams, "create needs a non-empty body"))?;
        let anchor = input.node.map(|node| CommentAnchor {
            node,
            find: input.anchor.as_ref().map(|a| a.find.clone()),
            nth: input.anchor.as_ref().and_then(|a| a.nth),
        });
        let comment = self
            .tools
            .service()
            .create_comment(
                who,
                CommentRequest {
                    document: input.document_id,
                    anchor,
                    body,
                    parent_id: input.parent_id,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        Ok(CommentOutput {
            comments: vec![comment.into()],
        })
    }

    async fn resolve(
        &self,
        who: &AuthenticatedUser,
        input: CommentInput,
        ctx: &RequestContext,
    ) -> Result<CommentOutput, McpError> {
        let comment = input
            .comment_id
            .ok_or_else(|| refusal(RefusalCode::InvalidParams, "resolve needs comment_id"))?;
        let resolved = self
            .tools
            .service()
            .resolve_comment(
                who,
                ResolveCommentRequest {
                    document: input.document_id,
                    comment,
                    answering: input.answering,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        Ok(CommentOutput {
            comments: vec![resolved.into()],
        })
    }
}

impl McpToolHandler for CommentHandler<'_> {
    type Input = CommentInput;
    type Output = CommentOutput;

    fn tool_name(&self) -> &'static str {
        "documents_comment"
    }

    fn description(&self) -> &'static str {
        "Create, list or resolve anchored comments. Guide topic: comments."
    }

    async fn handle(
        &self,
        input: CommentInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(CommentOutput, String), McpError> {
        let who = caller(ctx)?;
        let output = match input.action {
            CommentAction::Create => self.create(who, input, ctx).await?,
            CommentAction::Resolve => self.resolve(who, input, ctx).await?,
            CommentAction::List => CommentOutput {
                comments: self
                    .tools
                    .service()
                    .comments(who, &input.document_id, ctx.trace_id())
                    .await
                    .map_err(tool_error)?
                    .into_iter()
                    .map(CommentItem::from)
                    .collect(),
            },
        };
        let summary = format!("{} comments", output.comments.len());
        Ok((output, summary))
    }
}
