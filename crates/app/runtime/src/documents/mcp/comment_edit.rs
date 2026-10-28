//! The `edit` and `delete` actions of `documents_comment`: an author
//! rewrites a comment's body and mentions, and a comment is deleted by its
//! author or by a caller who may share the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::service::{DeleteCommentRequest, EditCommentRequest};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::execution::context::RequestContext;

use super::comment::CommentHandler;
use super::inputs::CommentInput;
use super::refusal::refusal;
use super::support::tool_error;
use crate::documents::refusal::RefusalCode;
use crate::documents::views::{CommentOutput, comments_view};

impl CommentHandler<'_> {
    pub(super) async fn edit(
        &self,
        who: &AuthenticatedUser,
        input: CommentInput,
        ctx: &RequestContext,
    ) -> Result<CommentOutput, McpError> {
        let comment = input
            .comment_id
            .ok_or_else(|| refusal(RefusalCode::InvalidParams, "edit needs comment_id"))?;
        let body = input
            .body
            .filter(|b| !b.trim().is_empty())
            .ok_or_else(|| refusal(RefusalCode::InvalidParams, "edit needs a non-empty body"))?;
        let edited = self
            .tools
            .service()
            .edit_comment(
                who,
                EditCommentRequest {
                    document: input.document_id,
                    comment,
                    body,
                    mentions: input.mentions,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        Ok(CommentOutput {
            comments: vec![edited.into()],
        })
    }

    pub(super) async fn delete(
        &self,
        who: &AuthenticatedUser,
        input: CommentInput,
        ctx: &RequestContext,
    ) -> Result<CommentOutput, McpError> {
        let comment = input
            .comment_id
            .ok_or_else(|| refusal(RefusalCode::InvalidParams, "delete needs comment_id"))?;
        let service = self.tools.service();
        service
            .delete_comment(
                who,
                DeleteCommentRequest {
                    document: input.document_id.clone(),
                    comment,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        comments_view(service, who, &input.document_id, ctx.trace_id())
            .await
            .map_err(tool_error)
    }
}
