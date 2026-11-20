//! The `documents_folder` tool: create a folder, or rename and move one.
//!
//! `create` makes a folder under `parent_id` (absent or null for the
//! root) and needs create access on that parent. `update` renames the
//! folder (edit access) and moves it (`parent_id`, null for the root) in
//! one transaction, so a refusal leaves it as it was. A move is an
//! administrative action: an agent may take it only where the agent access
//! policy allows it. Subfolders are listed by `documents_list`; deleting a
//! folder and setting its agent edit policy are not offered to agents.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::DocumentFolder;
use systemprompt_files::documents::service::{FolderMove, FolderUpdateRequest, NewFolderRequest};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{FolderAction, FolderInput};
use super::outputs::FolderItem;
use super::refusal::refusal;
use super::support::{caller, tool_error};
use crate::documents::refusal::RefusalCode;

fn folder_item(folder: DocumentFolder) -> FolderItem {
    FolderItem {
        folder_id: folder.id,
        parent_id: folder.parent_id,
        name: folder.name,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FolderHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl FolderHandler<'_> {
    async fn create(
        &self,
        input: FolderInput,
        ctx: &RequestContext,
    ) -> Result<(FolderItem, String), McpError> {
        let who = caller(ctx)?;
        if input.folder_id.is_some() {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "create takes parent_id and name; folder_id names the folder an update changes",
            ));
        }
        let Some(name) = input.name else {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "create needs the folder's name",
            ));
        };
        let parent = match input.parent_id {
            FolderMove::Keep | FolderMove::Root => None,
            FolderMove::Into(parent) => Some(parent),
        };
        let folder = self
            .tools
            .service()
            .create_folder(who, NewFolderRequest { parent, name }, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = format!("created folder {} ({})", folder.name, folder.id);
        Ok((folder_item(folder), summary))
    }

    async fn update(
        &self,
        input: FolderInput,
        ctx: &RequestContext,
    ) -> Result<(FolderItem, String), McpError> {
        let who = caller(ctx)?;
        let Some(folder) = input.folder_id else {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "update needs folder_id",
            ));
        };
        let request = FolderUpdateRequest {
            name: input.name,
            parent: input.parent_id,
            agent_edit_policy: None,
        };
        if request.is_empty() {
            return Err(refusal(
                RefusalCode::InvalidParams,
                "update needs a new name, a parent_id (null for the root), or both",
            ));
        }
        let folder = self
            .tools
            .service()
            .update_folder(who, &folder, request, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = format!("updated folder {} ({})", folder.name, folder.id);
        Ok((folder_item(folder), summary))
    }
}

impl McpToolHandler for FolderHandler<'_> {
    type Input = FolderInput;
    type Output = FolderItem;

    fn tool_name(&self) -> &'static str {
        "documents_folder"
    }

    fn description(&self) -> &'static str {
        "Create, rename or move a folder."
    }

    async fn handle(
        &self,
        input: FolderInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(FolderItem, String), McpError> {
        match input.action {
            FolderAction::Create => self.create(input, ctx).await,
            FolderAction::Update => self.update(input, ctx).await,
        }
    }
}
