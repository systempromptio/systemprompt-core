//! The document store's MCP tools.
//!
//! [`DocumentsMcpTools`](crate::documents::mcp::DocumentsMcpTools) is what an
//! extension's MCP server binary composes:
//! [`DocumentsMcpTools::list_tools`](crate::documents::mcp::DocumentsMcpTools::list_tools) for `tools/list`,
//! [`DocumentsMcpTools::call`](crate::documents::mcp::DocumentsMcpTools::call)
//! to run one tool through core's
//! [`McpToolExecutor`](systemprompt_mcp::McpToolExecutor) after the binary has
//! authenticated the request, and the `ui://<server>/document-preview` resource
//! for `resources/list` and `resources/read`. Every handler acts as the
//! authenticated user on the request context, and every write records the MCP
//! execution as the revision's origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod browse;
mod download;
mod export;
mod guide;
mod inputs;
mod lines;
mod listing;
mod outputs;
mod preview;
mod read;
mod refusal;
mod share;
mod support;
mod write;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListResourcesResult, ReadResourceRequestParams,
    ReadResourceResult, Tool,
};
use systemprompt_files::documents::DocumentService;
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::{ClientProfile, McpToolExecutor, McpToolHandler};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::execution::context::RequestContext;

pub use browse::{HistoryHandler, ListHandler, SearchHandler};
pub use download::{
    DOWNLOAD_AUDIENCE, DOWNLOAD_TOKEN_QUERY, DOWNLOAD_TTL, DownloadTarget, download_purpose,
    signed_download_url,
};
pub use export::ExportHandler;
pub use guide::GuideHandler;
pub use inputs::{
    CreateInput, EditInput, ExportAs, ExportFormat, ExportInput, GuideInput, HistoryInput,
    ListInput, NodeRange, ReadInput, ReadView, SearchInput, ShareInput,
};
pub use lines::{
    MAX_LINES, MAX_WRITTEN, RenderedLines, parse_cell, render, split_cell_id, structured_line,
    written_lines,
};
pub use outputs::{
    CreateOutput, DocumentItem, EditOutput, ExportOutput, FolderItem, GuideOutput, HistoryOutput,
    ListOutput, ReadOutput, RevisionItem, SearchHit, SearchOutput, ShareOutput,
};
pub use preview::{
    PREVIEW_RESOURCE_NAME, preview_resource, preview_resource_list, preview_resource_uri,
    preview_template, preview_tool_meta, read_preview_resource,
};
pub use read::ReadHandler;
pub use refusal::{Refusal, RefusalCode, refusal, refusal_result};
pub use share::ShareHandler;
pub use support::tool_error;
pub use write::{CreateHandler, EditHandler};

use crate::AppContext;

pub const TOOL_NAMES: [&str; 9] = [
    "documents_list",
    "documents_search",
    "documents_create",
    "documents_read",
    "documents_edit",
    "documents_history",
    "documents_share",
    "documents_export",
    "documents_guide",
];

pub const INSTRUCTIONS: &str = "Shared documents (Word, Excel, PowerPoint, OpenDocument, HTML, \
Markdown, text) you may view, edit and share.
1. Read the outline first: documents_read view outline; lines are `<node>:<hash>| <text>`.
2. Edit by node id and hash, or by quote with str_replace, in one documents_edit call.
3. Anchor the next edit on the ack: its written lines and revision_id.
4. On stale_base or guard_mismatch their edit wins: re-aim from the returned text, never resend \
your old text.
5. Do not re-read between your own writes. Details: documents_guide.";

/// Why [`DocumentsMcpTools::from_context`] refused to build the tools.
#[derive(Debug, thiserror::Error)]
pub enum DocumentsMcpError {
    #[error(
        "MCP server {server_name} serves document tools but the document store is disabled: set \
         files.documents.enabled: true in services/config/files.yaml"
    )]
    Disabled { server_name: McpServerId },
}

/// What [`DocumentsMcpTools`] is built from.
#[derive(Clone)]
pub struct DocumentsMcpConfig {
    pub service: Arc<DocumentService>,
    pub tokens: Arc<dyn TokenAuthority>,
    pub server_name: McpServerId,
    pub api_external_url: String,
}

/// One tool call, already authenticated by the composing server.
#[derive(Debug, Clone, Copy)]
pub struct DocumentsToolCall<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub context: &'a RequestContext,
    pub client: &'a ClientProfile,
}

/// The nine document tools and the preview resource of one MCP server.
#[derive(Clone)]
pub struct DocumentsMcpTools {
    service: Arc<DocumentService>,
    tokens: Arc<dyn TokenAuthority>,
    server_name: McpServerId,
    api_external_url: String,
}

impl std::fmt::Debug for DocumentsMcpTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsMcpTools")
            .field("server_name", &self.server_name)
            .field("api_external_url", &self.api_external_url)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DocumentsMcpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsMcpConfig")
            .field("server_name", &self.server_name)
            .field("api_external_url", &self.api_external_url)
            .finish_non_exhaustive()
    }
}

impl DocumentsMcpTools {
    #[must_use]
    pub fn new(config: DocumentsMcpConfig) -> Self {
        Self {
            service: config.service,
            tokens: config.tokens,
            server_name: config.server_name,
            api_external_url: config.api_external_url,
        }
    }

    pub fn from_context(
        ctx: &AppContext,
        server_name: McpServerId,
    ) -> Result<Self, DocumentsMcpError> {
        if !ctx.files_config().documents_config().is_enabled() {
            return Err(DocumentsMcpError::Disabled { server_name });
        }
        Ok(Self::new(DocumentsMcpConfig {
            service: Arc::clone(ctx.documents().service()),
            tokens: ctx.token_authority(),
            server_name,
            api_external_url: ctx.config().api_external_url.clone(),
        }))
    }

    #[must_use]
    pub fn service(&self) -> &DocumentService {
        &self.service
    }

    #[must_use]
    pub fn token_authority(&self) -> &dyn TokenAuthority {
        self.tokens.as_ref()
    }

    #[must_use]
    pub const fn server_name(&self) -> &McpServerId {
        &self.server_name
    }

    #[must_use]
    pub fn api_external_url(&self) -> &str {
        &self.api_external_url
    }

    #[must_use]
    pub const fn tool_names() -> &'static [&'static str] {
        &TOOL_NAMES
    }

    #[must_use]
    pub fn list_tools(&self) -> Vec<Tool> {
        let server = &self.server_name;
        let mut export = ExportHandler { tools: self }.tool_definition(server);
        export.meta = Some(preview_tool_meta(server));
        vec![
            listing::compact(ListHandler { tools: self }.tool_definition(server), false),
            listing::compact(SearchHandler { tools: self }.tool_definition(server), false),
            listing::compact(CreateHandler { tools: self }.tool_definition(server), false),
            listing::compact(ReadHandler { tools: self }.tool_definition(server), false),
            listing::compact(EditHandler { tools: self }.tool_definition(server), false),
            listing::compact(
                HistoryHandler { tools: self }.tool_definition(server),
                false,
            ),
            listing::compact(ShareHandler { tools: self }.tool_definition(server), false),
            listing::compact(export, true),
            listing::compact(GuideHandler { tools: self }.tool_definition(server), false),
        ]
    }

    pub async fn call(&self, call: DocumentsToolCall<'_>) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "documents_list" => run(call, &ListHandler { tools: self }).await,
            "documents_search" => run(call, &SearchHandler { tools: self }).await,
            "documents_create" => run(call, &CreateHandler { tools: self }).await,
            "documents_read" => run(call, &ReadHandler { tools: self }).await,
            "documents_edit" => run(call, &EditHandler { tools: self }).await,
            "documents_history" => run(call, &HistoryHandler { tools: self }).await,
            "documents_share" => run(call, &ShareHandler { tools: self }).await,
            "documents_export" => run(call, &ExportHandler { tools: self }).await,
            "documents_guide" => run(call, &GuideHandler { tools: self }).await,
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown tool: '{other}'. This server exposes: {}.",
                    TOOL_NAMES.join(", ")
                ),
                None,
            )),
        }
    }

    #[must_use]
    pub fn list_resources(&self) -> ListResourcesResult {
        preview_resource_list(&self.server_name)
    }

    pub fn read_resource(
        &self,
        request: &ReadResourceRequestParams,
    ) -> Result<ReadResourceResult, McpError> {
        read_preview_resource(request, &self.server_name)
    }
}

async fn run<H: McpToolHandler>(
    call: DocumentsToolCall<'_>,
    handler: &H,
) -> Result<CallToolResult, McpError> {
    match call
        .executor
        .execute(handler, call.request, call.context, call.client)
        .await
    {
        Err(error) => refusal_result(&error).ok_or(error),
        ok => ok,
    }
}
