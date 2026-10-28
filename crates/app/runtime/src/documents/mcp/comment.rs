//! Comment threads anchor to a stable node id and an optional quote inside
//! it, so a thread survives edits elsewhere in the document and is detached,
//! not lost, when its words go. A comment may mention users by id or email
//! (`mentions`), which shares the document with any of them who cannot see
//! it; its author can edit it, and it can be deleted (`comment_edit`).
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
use super::mentions::{agent_origin, resolve_mentions};
use super::refusal::refusal;
use super::support::{caller, tool_error};
use crate::documents::refusal::RefusalCode;
use crate::documents::views::{CommentOutput, comments_view};

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
        let resolved = resolve_mentions(self.tools.users(), input.mentions, body).await?;
        let comment = self
            .tools
            .service()
            .create_comment_from(
                who,
                CommentRequest {
                    document: input.document_id,
                    anchor,
                    range: None,
                    body: resolved.body,
                    mentions: resolved.ids,
                    parent_id: input.parent_id,
                },
                &agent_origin(ctx),
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
        "Create, list, resolve, edit or delete anchored comments; mention users by id or email."
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
            CommentAction::Edit => self.edit(who, input, ctx).await?,
            CommentAction::Delete => self.delete(who, input, ctx).await?,
            CommentAction::List => comments_view(
                self.tools.service(),
                who,
                &input.document_id,
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?,
        };
        let summary = format!("{} comments", output.comments.len());
        Ok((output, summary))
    }
}
