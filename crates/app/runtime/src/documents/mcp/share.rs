//! Sharing a document or a folder subtree with a subject at an access level.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::service::ShareRequest;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::ShareInput;
use super::outputs::ShareOutput;
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

impl McpToolHandler for ShareHandler<'_> {
    type Input = ShareInput;
    type Output = ShareOutput;

    fn tool_name(&self) -> &'static str {
        "documents_share"
    }

    fn description(&self) -> &'static str {
        "Grant a subject access to a document or a folder subtree. subject_kind is user, role or \
         a registered subject dimension such as group or project; level is view, comment, edit, \
         share or own. Needs share access."
    }

    async fn handle(
        &self,
        input: ShareInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ShareOutput, String), McpError> {
        let who = caller(ctx)?;
        let target = share_target(&input)?;
        let grant = self
            .tools
            .service()
            .share(
                who,
                ShareRequest {
                    target,
                    subject_kind: input.subject_kind,
                    subject_value: input.subject_value,
                    level: input.level,
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
                grant_id: grant.id.to_string(),
                subject_kind: grant.subject_kind,
                subject_value: grant.subject_value,
                level: grant.access_level,
                valid_until: grant.valid_until,
            },
            summary,
        ))
    }
}
