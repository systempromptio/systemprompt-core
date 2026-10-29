//! The MCP tool through which an agent works its user's notification feed,
//! recorded against the MCP surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{McpExecutionId, NotificationId};
use systemprompt_mcp::{McpOutputSchema, McpToolHandler};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::notifications::{
    Attention, Escalate, KeysetCursor, MarkRead, Notification, NotificationPage, SURFACE_MCP,
};
use systemprompt_notifications::NotificationsError;
use systemprompt_notifications::service::FeedQuery;
use systemprompt_traits::RepositoryError;

use super::DocumentsMcpTools;
use super::refusal::refusal;
use super::support::caller;
use crate::documents::refusal::RefusalCode;

const DEFAULT_LIMIT: u32 = 50;

const MAX_LIMIT: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InboxAction {
    List,
    MarkRead,
    Escalate,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct InboxInput {
    pub action: InboxAction,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub attention: Option<Attention>,
    #[serde(default)]
    pub unread: Option<bool>,
    #[serde(default)]
    pub ids: Vec<NotificationId>,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub id: Option<NotificationId>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum InboxOutput {
    Page(NotificationPage),
    Read {
        ids: Vec<NotificationId>,
        unread: i64,
    },
    Escalated(Box<Notification>),
}

impl McpOutputSchema for InboxOutput {
    fn artifact_type() -> &'static str {
        "documents_inbox"
    }

    fn terse_wire(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InboxHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

fn inbox_error(error: NotificationsError) -> McpError {
    match error {
        NotificationsError::NotFound { .. }
        | NotificationsError::SubscriptionNotFound { .. }
        | NotificationsError::StreamNotFound { .. } => {
            refusal(RefusalCode::NotFound, error.to_string())
        },
        NotificationsError::Repository(ref repo @ RepositoryError::NotFound { .. }) => {
            refusal(RefusalCode::NotFound, repo.to_string())
        },
        NotificationsError::PublishRefused { .. } => {
            refusal(RefusalCode::Forbidden, error.to_string())
        },
        NotificationsError::NotEscalatable { .. }
        | NotificationsError::InvalidDeclaration(_)
        | NotificationsError::Kind(_) => refusal(RefusalCode::InvalidParams, error.to_string()),
        NotificationsError::Repository(
            ref repo @ (RepositoryError::Conflict { .. }
            | RepositoryError::InvalidArgument { .. }
            | RepositoryError::InvalidData { .. }),
        ) => refusal(RefusalCode::InvalidParams, repo.to_string()),
        other => {
            tracing::error!(
                error = %other,
                cause = ?std::error::Error::source(&other),
                "inbox tool failed"
            );
            McpError::internal_error("the inbox failed; try the call again", None)
        },
    }
}

impl InboxHandler<'_> {
    async fn list(&self, ctx: &RequestContext, input: InboxInput) -> Result<InboxOutput, McpError> {
        let since = input
            .since
            .as_deref()
            .map(|raw| {
                raw.parse::<KeysetCursor>().map_err(|error| {
                    tracing::debug!(error = %error, since = raw, "inbox cursor rejected");
                    refusal(
                        RefusalCode::InvalidParams,
                        format!("since \"{raw}\" is not valid; list again without it"),
                    )
                })
            })
            .transpose()?;
        let query = FeedQuery {
            since,
            limit: input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
            attention: input.attention,
            unread_only: input.unread.unwrap_or(false),
        };
        let page = self
            .tools
            .notifications()
            .feed(ctx.user_id(), query)
            .await
            .map_err(inbox_error)?;
        Ok(InboxOutput::Page(page))
    }

    async fn mark_read(
        &self,
        ctx: &RequestContext,
        input: InboxInput,
    ) -> Result<InboxOutput, McpError> {
        if input.ids.is_empty() && !input.all {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "mark_read needs ids or all: true",
            ));
        }
        let outcome = self
            .tools
            .notifications()
            .mark_read(
                ctx.user_id(),
                MarkRead {
                    ids: input.ids,
                    all: input.all,
                },
                SURFACE_MCP,
            )
            .await
            .map_err(inbox_error)?;
        Ok(InboxOutput::Read {
            ids: outcome.ids,
            unread: outcome.unread,
        })
    }

    async fn escalate(
        &self,
        ctx: &RequestContext,
        input: InboxInput,
    ) -> Result<InboxOutput, McpError> {
        let id = input
            .id
            .ok_or_else(|| refusal(RefusalCode::InvalidParams, "escalate needs id"))?;
        let escalated = self
            .tools
            .notifications()
            .escalate(
                ctx.user_id(),
                &id,
                Escalate { note: input.note },
                SURFACE_MCP,
            )
            .await
            .map_err(inbox_error)?;
        Ok(InboxOutput::Escalated(Box::new(escalated)))
    }
}

impl McpToolHandler for InboxHandler<'_> {
    type Input = InboxInput;
    type Output = InboxOutput;

    fn tool_name(&self) -> &'static str {
        "documents_inbox"
    }

    fn description(&self) -> &'static str {
        "Your user's notifications: list, mark_read, escalate. Guide: inbox."
    }

    async fn handle(
        &self,
        input: InboxInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(InboxOutput, String), McpError> {
        caller(ctx)?;
        let output = match input.action {
            InboxAction::List => self.list(ctx, input).await?,
            InboxAction::MarkRead => self.mark_read(ctx, input).await?,
            InboxAction::Escalate => self.escalate(ctx, input).await?,
        };
        let summary = match &output {
            InboxOutput::Page(page) => format!(
                "{} notifications, {} unread",
                page.notifications.len(),
                page.unread
            ),
            InboxOutput::Read { ids, unread } => {
                format!("{} marked read, {unread} unread", ids.len())
            },
            InboxOutput::Escalated(_) => "escalated to human".to_owned(),
        };
        Ok((output, summary))
    }
}
