//! Issue tool refusals.
//!
//! A refusal is an [`McpError`] whose `data` carries a `code`, turned into an
//! `isError` tool result at the dispatch boundary; [`tool_error`] maps
//! [`IssuesError`] onto those codes. Infrastructure failures are
//! logged with their cause and answered with a fixed message.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;
use systemprompt_files::issues::IssuesError;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::execution::context::RequestContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueRefusalCode {
    Forbidden,
    Disabled,
    InvalidParams,
    RateLimited,
    NotFound,
    NotActive,
    AttachmentsFull,
    UploadNotReady,
    UploadsUnavailable,
}

#[must_use]
pub fn refusal(code: IssueRefusalCode, message: impl Into<String>) -> McpError {
    // JSON: a refusal's structuredContent is its free-form contract field.
    McpError::invalid_params(message.into(), Some(serde_json::json!({ "code": code })))
}

#[must_use]
pub fn refusal_result(error: &McpError) -> Option<CallToolResult> {
    let data = error
        .data
        .as_ref()
        .filter(|data| data.get("code").is_some_and(serde_json::Value::is_string))?;
    let mut result = CallToolResult::error(vec![ContentBlock::text(error.message.to_string())]);
    result.structured_content = Some(data.clone());
    Some(result)
}

#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "handlers pass it as the fn pointer of map_err over an owned IssuesError"
)]
pub fn tool_error(error: IssuesError) -> McpError {
    let code = match &error {
        IssuesError::Disabled => IssueRefusalCode::Disabled,
        IssuesError::Invalid { .. } => IssueRefusalCode::InvalidParams,
        IssuesError::RateLimited { .. } => IssueRefusalCode::RateLimited,
        IssuesError::NotFound(_) => IssueRefusalCode::NotFound,
        IssuesError::NotActive(_) => IssueRefusalCode::NotActive,
        IssuesError::AttachmentsFull { .. } => IssueRefusalCode::AttachmentsFull,
        IssuesError::UploadNotReady(_) => IssueRefusalCode::UploadNotReady,
        IssuesError::Repository(_)
        | IssuesError::Database(_)
        | IssuesError::Notifications(_)
        | IssuesError::NotificationKind(_) => {
            tracing::error!(
                error = %error,
                cause = ?std::error::Error::source(&error),
                "issue tool failed"
            );
            return McpError::internal_error("the issue store failed; try the call again", None);
        },
    };
    refusal(code, error.to_string())
}

pub fn caller(ctx: &RequestContext) -> Result<&AuthenticatedUser, McpError> {
    ctx.user.as_ref().ok_or_else(|| {
        refusal(
            IssueRefusalCode::Forbidden,
            "issue tools act as the signed-in user; this call carried no authenticated user",
        )
    })
}
