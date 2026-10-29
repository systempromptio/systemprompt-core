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

mod access;
mod browse;
mod catalog;
mod comment;
mod comment_edit;
mod export;
mod guide;
mod guide_model;
mod guide_topics;
#[cfg(feature = "notifications")]
mod inbox;
mod inputs;
mod lifecycle;
mod listing;
mod media;
mod mentions;
mod outputs;
mod people;
mod preview;
mod proposals;
mod read;
mod records;
mod refusal;
mod share;
mod support;
mod upload;
mod write;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListResourcesResult, ReadResourceRequestParams,
    ReadResourceResult, Tool,
};
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::agents::act_as;
use systemprompt_files::uploads::UploadService;
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::{ClientProfile, McpToolExecutor};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::execution::context::RequestContext;
#[cfg(feature = "notifications")]
use systemprompt_notifications::service::NotificationsService;
use systemprompt_users::UserRepository;

pub use access::AccessHandler;
pub use browse::{HistoryHandler, ListHandler, SearchHandler};
pub use catalog::{INSTRUCTIONS, TOOL_NAMES};
pub use comment::CommentHandler;
pub use export::ExportHandler;
pub use guide::GuideHandler;
#[cfg(feature = "notifications")]
pub use inbox::{InboxAction, InboxHandler, InboxInput, InboxOutput};
pub use inputs::{
    AccessInput, CommentAction, CommentInput, CopyInput, CreateInput, DocumentInput, EditInput,
    ExportAs, ExportFormat, ExportInput, GuideInput, HistoryInput, ListInput, ListScopeInput,
    ListSortInput, MentionOptionInput, PeopleInput, ProposalAction, ProposalsInput, QuoteAnchor,
    SearchInput, ShareInput, UpdateInput,
};
pub use lifecycle::{CopyHandler, TrashHandler, UntrashHandler, UpdateHandler};
pub use outputs::{
    CreateOutput, DocumentItem, ExportOutput, FolderItem, GuideOutput, HistoryOutput, ListOutput,
    PeopleOutput, PersonItem, ProposalItem, ProposalsOutput, ReviewOutput, RevisionAttributionItem,
    RevisionItem, SearchHit, SearchOutput, ShareOutput,
};
pub use people::PeopleHandler;
pub use preview::{
    PREVIEW_RESOURCE_NAME, preview_resource, preview_resource_list, preview_resource_uri,
    preview_template, preview_tool_meta, read_preview_resource,
};
pub use proposals::ProposalsHandler;
pub use read::ReadHandler;
pub use records::DocumentAck;
pub use refusal::{refusal, refusal_code, refusal_result, refused_error};
pub use share::ShareHandler;
pub use support::tool_error;
pub use upload::{UploadHandler, UploadInput, UploadMode, UploadOutput};
pub use write::{CreateHandler, EditHandler, create_content};

use support::run;

use crate::AppContext;

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
    #[cfg(feature = "notifications")]
    pub notifications: Arc<NotificationsService>,
    pub uploads: Option<Arc<UploadService>>,
    pub users: Arc<UserRepository>,
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

/// The nineteen document tools and the preview resource of one MCP server.
#[derive(Clone)]
pub struct DocumentsMcpTools {
    service: Arc<DocumentService>,
    #[cfg(feature = "notifications")]
    notifications: Arc<NotificationsService>,
    uploads: Option<Arc<UploadService>>,
    users: Arc<UserRepository>,
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
            #[cfg(feature = "notifications")]
            notifications: config.notifications,
            uploads: config.uploads,
            users: config.users,
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
            #[cfg(feature = "notifications")]
            notifications: Arc::clone(&ctx.notifications().service),
            uploads: Some(Arc::clone(ctx.documents().uploads())),
            users: Arc::clone(ctx.user_repository()),
            tokens: ctx.token_authority(),
            server_name,
            api_external_url: ctx.config().api_external_url.clone(),
        }))
    }

    #[must_use]
    pub fn service(&self) -> &DocumentService {
        &self.service
    }

    #[cfg(feature = "notifications")]
    #[must_use]
    pub fn notifications(&self) -> &NotificationsService {
        &self.notifications
    }

    #[must_use]
    pub fn uploads(&self) -> Option<&UploadService> {
        self.uploads.as_deref()
    }

    #[must_use]
    pub fn users(&self) -> &UserRepository {
        &self.users
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
        listing::tool_list(self)
    }

    pub async fn call(&self, call: DocumentsToolCall<'_>) -> Result<CallToolResult, McpError> {
        act_as(mentions::document_actor(call.context), self.dispatch(call)).await
    }

    async fn dispatch(&self, call: DocumentsToolCall<'_>) -> Result<CallToolResult, McpError> {
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
            "documents_trash" => Box::pin(run(call, &TrashHandler { tools: self })).await,
            "documents_untrash" => Box::pin(run(call, &UntrashHandler { tools: self })).await,
            "documents_export" => Box::pin(run(call, &ExportHandler { tools: self })).await,
            "documents_upload" => Box::pin(run(call, &UploadHandler { tools: self })).await,
            "documents_guide" => Box::pin(run(call, &GuideHandler { tools: self })).await,
            "documents_people" => Box::pin(run(call, &PeopleHandler { tools: self })).await,
            #[cfg(feature = "notifications")]
            "documents_inbox" => Box::pin(run(call, &InboxHandler { tools: self })).await,
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
