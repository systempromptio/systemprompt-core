//! Dispatch of one tool call by name to its handler, run through core's
//! executor; an unknown name lists the tools this server exposes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::CallToolResult;

use super::support::run;
use super::{
    AccessHandler, CommentHandler, CopyHandler, CreateHandler, DocumentsMcpTools,
    DocumentsToolCall, EditHandler, ExportHandler, FolderHandler, GuideHandler, HistoryHandler,
    ImportHandler, InboxHandler, ListHandler, PeopleHandler, ProposalsHandler, ReadHandler,
    SearchHandler, ShareHandler, TOOL_NAMES, TrashHandler, UntrashHandler, UpdateHandler,
    UploadHandler,
};

impl DocumentsMcpTools {
    pub(super) async fn dispatch(
        &self,
        call: DocumentsToolCall<'_>,
    ) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "documents_list" => Box::pin(run(call, &ListHandler { tools: self })).await,
            "documents_search" => Box::pin(run(call, &SearchHandler { tools: self })).await,
            "documents_create" => Box::pin(run(call, &CreateHandler { tools: self })).await,
            "documents_read" => Box::pin(run(call, &ReadHandler { tools: self })).await,
            "documents_edit" => Box::pin(run(call, &EditHandler { tools: self })).await,
            "documents_comment" => Box::pin(run(call, &CommentHandler { tools: self })).await,
            "documents_history" => Box::pin(run(call, &HistoryHandler { tools: self })).await,
            "documents_proposals" => Box::pin(run(call, &ProposalsHandler { tools: self })).await,
            "documents_share" => Box::pin(run(call, &ShareHandler { tools: self })).await,
            "documents_access" => Box::pin(run(call, &AccessHandler { tools: self })).await,
            "documents_copy" => Box::pin(run(call, &CopyHandler { tools: self })).await,
            "documents_update" => Box::pin(run(call, &UpdateHandler { tools: self })).await,
            "documents_folder" => Box::pin(run(call, &FolderHandler { tools: self })).await,
            "documents_trash" => Box::pin(run(call, &TrashHandler { tools: self })).await,
            "documents_untrash" => Box::pin(run(call, &UntrashHandler { tools: self })).await,
            "documents_export" => {
                Box::pin(run(
                    call,
                    &ExportHandler {
                        tools: self,
                        hop: call.hop,
                    },
                ))
                .await
            },
            "documents_upload" => {
                Box::pin(run(
                    call,
                    &UploadHandler {
                        tools: self,
                        hop: call.hop,
                    },
                ))
                .await
            },
            "documents_import" => Box::pin(run(call, &ImportHandler { tools: self })).await,
            "documents_guide" => Box::pin(run(call, &GuideHandler { tools: self })).await,
            "documents_people" => Box::pin(run(call, &PeopleHandler { tools: self })).await,
            "documents_inbox" if self.notifications.is_some() => {
                Box::pin(run(call, &InboxHandler { tools: self })).await
            },
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown tool: '{other}'. This server exposes: {}.",
                    TOOL_NAMES.join(", ")
                ),
                None,
            )),
        }
    }
}
