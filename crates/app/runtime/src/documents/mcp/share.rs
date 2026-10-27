//! Sharing a document or a folder subtree with a subject at an access level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::service::ShareRequest;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::execution::context::RequestContext;

use crate::documents::subject_display;

use super::DocumentsMcpTools;
use super::inputs::ShareInput;
use super::outputs::ShareOutput;
use super::records::GrantItem;
use super::refusal::{RefusalCode, refusal};
use super::support::{caller, tool_error};

#[derive(Debug, Clone, Copy)]
pub struct ShareHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

fn share_target(input: &ShareInput) -> Result<GrantTarget, McpError> {
    match (&input.document_id, &input.folder_id) {
        (Some(document), None) => Ok(GrantTarget::Document(document.clone())),
        (None, Some(folder)) => Ok(GrantTarget::Folder(folder.clone())),
        _ => Err(McpError::invalid_params(
            "share exactly one of document_id or folder_id",
            None,
        )),
    }
}

impl ShareHandler<'_> {
    async fn grant(
        &self,
        who: &AuthenticatedUser,
        input: ShareInput,
        ctx: &RequestContext,
    ) -> Result<(ShareOutput, String), McpError> {
        let target = share_target(&input)?;
        let (Some(subject_kind), Some(subject_value), Some(level)) =
            (input.subject_kind, input.subject_value, input.level)
        else {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "a grant needs subject_kind, subject_value and level",
            ));
        };
        let display = subject_display(self.tools.users(), &subject_kind, &subject_value)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "grant subject was not looked up");
                McpError::internal_error("the grant subject could not be looked up", None)
            })?;
        let grant = self
            .tools
            .service()
            .share(
                who,
                ShareRequest {
                    target,
                    subject_kind,
                    subject_value,
                    subject_display: Some(display),
                    level,
                    valid_until: input.valid_until,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let summary = format!(
            "granted {} to {} {}",
            grant.access_level, grant.subject_kind, grant.subject_value
        );
        Ok((
            ShareOutput {
                grant: Some(GrantItem::from(grant)),
                transferred_to: None,
                revoked: None,
            },
            summary,
        ))
    }
}

impl McpToolHandler for ShareHandler<'_> {
    type Input = ShareInput;
    type Output = ShareOutput;

    fn tool_name(&self) -> &'static str {
        "documents_share"
    }

    fn description(&self) -> &'static str {
        "Grant, transfer ownership or revoke access."
    }

    async fn handle(
        &self,
        input: ShareInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ShareOutput, String), McpError> {
        let who = caller(ctx)?;
        let service = self.tools.service();
        let trace = ctx.trace_id();
        match (
            input.transfer_to.clone(),
            input.revoke_all,
            &input.document_id,
        ) {
            (Some(to), _, Some(document)) => {
                service
                    .transfer_ownership(who, document, &to, trace)
                    .await
                    .map_err(tool_error)?;
                let summary = format!("ownership transferred to {to}");
                Ok((
                    ShareOutput {
                        grant: None,
                        transferred_to: Some(to),
                        revoked: None,
                    },
                    summary,
                ))
            },
            (None, true, Some(document)) => {
                let revoked = service
                    .revoke_all(who, document, trace)
                    .await
                    .map_err(tool_error)?;
                Ok((
                    ShareOutput {
                        grant: None,
                        transferred_to: None,
                        revoked: Some(revoked),
                    },
                    format!("revoked {revoked} grants"),
                ))
            },
            (Some(_), _, None) | (None, true, None) => Err(refusal(
                RefusalCode::InvalidParams,
                "transfer_to and revoke_all take a document_id",
            )),
            (None, false, _) => self.grant(who, input, ctx).await,
        }
    }
}
