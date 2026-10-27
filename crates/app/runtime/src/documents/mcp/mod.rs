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
mod inputs;
mod lines;
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
pub use inputs::{
    CreateInput, EditInput, HistoryInput, ListInput, NodeRange, ReadInput, ReadView, RevisionInput,
    SearchInput, SetCellsInput, ShareInput, StrReplaceInput,
};
pub use lines::{MAX_LINES, RenderedLines, parse_cell, render, split_cell_id, structured_line};
pub use outputs::{
    CreateOutput, DocumentItem, DownloadOutput, EditOutput, FolderItem, HistoryOutput, ListOutput,
    PreviewOutput, ReadOutput, RevisionItem, SearchHit, SearchOutput, ShareOutput,
};
pub use preview::{
    PREVIEW_RESOURCE_NAME, preview_resource, preview_resource_list, preview_resource_uri,
    preview_template, preview_tool_meta, read_preview_resource,
};
pub use refusal::{Refusal, RefusalCode, refusal, refusal_result};
pub use read::{DownloadUrlHandler, ReadHandler, RenderPreviewHandler};
pub use share::ShareHandler;
pub use support::tool_error;
pub use write::{CreateHandler, EditHandler, SetCellsHandler, StrReplaceHandler};

use crate::AppContext;

pub const TOOL_NAMES: [&str; 11] = [
    "documents_list",
    "documents_search",
    "documents_create",
    "documents_read",
    "documents_edit",
    "documents_str_replace",
    "documents_set_cells",
    "documents_history",
    "documents_share",
    "documents_download_url",
    "documents_render_preview",
];

pub const INSTRUCTIONS: &str = "Shared documents (Word, Excel, PowerPoint, OpenDocument, HTML, \
    Markdown, text) you may view, edit and share. Find documents with documents_list or \
    documents_search, then documents_read (view outline first for large documents). Each read \
    line is `<node>:<hash>| <text>`; edit with documents_edit, documents_str_replace or \
    documents_set_cells, passing the read's revision_id as base_revision and the node's hash \
    as expected_hash. A stale hash or base is refused with the current text or head revision: \
    re-read and resend. Locations are document://<document>@<revision>#<node> URIs.";

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

/// The eleven document tools and the preview resource of one MCP server.
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
        let mut preview = RenderPreviewHandler { tools: self }.tool_definition(server);
        preview.meta = Some(preview_tool_meta(server));
        vec![
            ListHandler { tools: self }.tool_definition(server),
            SearchHandler { tools: self }.tool_definition(server),
            CreateHandler { tools: self }.tool_definition(server),
            ReadHandler { tools: self }.tool_definition(server),
            EditHandler { tools: self }.tool_definition(server),
            StrReplaceHandler { tools: self }.tool_definition(server),
            SetCellsHandler { tools: self }.tool_definition(server),
            HistoryHandler { tools: self }.tool_definition(server),
            ShareHandler { tools: self }.tool_definition(server),
            DownloadUrlHandler { tools: self }.tool_definition(server),
            preview,
        ]
    }

    pub async fn call(&self, call: DocumentsToolCall<'_>) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "documents_list" => run(call, &ListHandler { tools: self }).await,
            "documents_search" => run(call, &SearchHandler { tools: self }).await,
            "documents_create" => run(call, &CreateHandler { tools: self }).await,
            "documents_read" => run(call, &ReadHandler { tools: self }).await,
            "documents_edit" => run(call, &EditHandler { tools: self }).await,
            "documents_str_replace" => run(call, &StrReplaceHandler { tools: self }).await,
            "documents_set_cells" => run(call, &SetCellsHandler { tools: self }).await,
            "documents_history" => run(call, &HistoryHandler { tools: self }).await,
            "documents_share" => run(call, &ShareHandler { tools: self }).await,
            "documents_download_url" => run(call, &DownloadUrlHandler { tools: self }).await,
            "documents_render_preview" => run(call, &RenderPreviewHandler { tools: self }).await,
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
