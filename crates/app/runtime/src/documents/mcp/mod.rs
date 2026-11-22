//! The document store's MCP tools.
//!
//! [`DocumentsMcpTools`](crate::documents::mcp::DocumentsMcpTools) is what an
//! extension's MCP server binary composes:
//! [`DocumentsMcpTools::list_tools`](crate::documents::mcp::DocumentsMcpTools::list_tools) for `tools/list`,
//! [`DocumentsMcpTools::call`](crate::documents::mcp::DocumentsMcpTools::call)
//! to run one tool through core's
//! the MCP member's `ToolExecution` after the binary has
//! authenticated the request. Every handler acts as the authenticated user on
//! the request context, and every write records the MCP execution as the
//! revision's origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;
mod agent_presence;
mod browse;
mod catalog;
mod comment;
mod comment_edit;
mod create;
mod dispatch;
mod export;
mod folders;
mod guide;
mod guide_model;
mod guide_refusals;
mod guide_topics;
mod import;
#[cfg(feature = "notifications")]
mod inbox;
mod inputs;
mod lifecycle;
mod lifecycle_inputs;
mod listing;
mod live_edit;
mod media;
mod mentions;
mod op_inputs;
mod outputs;
mod people;
mod proposals;
mod read;
mod records;
mod refusal;
mod share;
mod support;
mod upload;
mod write;

use std::sync::Arc;
use systemprompt_extension::mcp::ToolExecution;

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::agents::act_as;
use systemprompt_files::uploads::UploadService;
use systemprompt_identifiers::{JwtToken, McpServerId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::mcp::ClientProfile;
#[cfg(feature = "notifications")]
use systemprompt_notifications::service::{NotificationsService, NotificationsServiceSlot};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::DynContextDirectory;
use systemprompt_users::UserRepository;

pub use access::AccessHandler;
pub use browse::{HistoryHandler, ListHandler, SearchHandler};
pub use catalog::{INSTRUCTIONS, TOOL_NAMES};
pub use comment::CommentHandler;
pub use create::{CreateHandler, create_content};
pub use export::ExportHandler;
pub use folders::FolderHandler;
pub use guide::GuideHandler;
pub use import::ImportHandler;
#[cfg(feature = "notifications")]
pub use inbox::{InboxAction, InboxHandler, InboxInput, InboxOutput};
pub use inputs::{
    AccessInput, CommentAction, CommentInput, CopyInput, CreateInput, DocumentInput, EditInput,
    ExportAs, ExportFormat, ExportInput, FolderAction, FolderInput, GuideInput, HistoryInput,
    ImportInput, ListInput, ListScopeInput, ListSortInput, MentionOptionInput, PeopleInput,
    ProposalAction, ProposalsInput, QuoteAnchor, SearchInput, ShareInput, UpdateInput,
};
pub use lifecycle::{CopyHandler, TrashHandler, UntrashHandler, UpdateHandler};
pub use outputs::{
    CreateOutput, DocumentItem, ExportOutput, FolderItem, GuideOutput, HistoryOutput, ListOutput,
    ProposalItem, ProposalsOutput, ReviewOutput, RevisionAttributionItem, RevisionItem, SearchHit,
    SearchOutput, ShareOutput,
};
pub use people::PeopleHandler;
pub use proposals::ProposalsHandler;
pub use read::ReadHandler;
pub use records::DocumentAck;
pub use refusal::{refusal, refusal_code, refusal_result, refused_error};
pub use share::ShareHandler;
pub use support::tool_error;
pub use upload::{UploadHandler, UploadInput, UploadMode, UploadOutput};
pub use write::EditHandler;

use crate::documents::links::LinkIssuer;

pub const TOOLSET_ID: &str = "documents";

/// What [`DocumentsMcpTools`] is built from.
#[derive(Clone)]
pub struct DocumentsMcpConfig {
    pub service: Arc<DocumentService>,
    #[cfg(feature = "notifications")]
    pub notifications: Option<Arc<NotificationsService>>,
    pub uploads: Option<Arc<UploadService>>,
    pub users: Arc<UserRepository>,
    pub links: Arc<dyn LinkIssuer>,
    pub contexts: DynContextDirectory,
    pub server_name: McpServerId,
}

/// One tool call, already authenticated by the composing server.
#[derive(Debug, Clone, Copy)]
pub struct DocumentsToolCall<'a> {
    pub executor: &'a dyn ToolExecution,
    pub request: &'a CallToolRequestParams,
    pub context: &'a RequestContext,
    pub client: &'a ClientProfile,
    pub hop: Option<&'a JwtToken>,
}

/// The twenty-one document tools and the preview resource of one MCP server.
#[derive(Clone)]
pub struct DocumentsMcpTools {
    service: Arc<DocumentService>,
    #[cfg(feature = "notifications")]
    notifications: Option<Arc<NotificationsService>>,
    uploads: Option<Arc<UploadService>>,
    users: Arc<UserRepository>,
    links: Arc<dyn LinkIssuer>,
    contexts: DynContextDirectory,
    server_name: McpServerId,
}

impl std::fmt::Debug for DocumentsMcpTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsMcpTools")
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DocumentsMcpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsMcpConfig")
            .field("server_name", &self.server_name)
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
            links: config.links,
            contexts: config.contexts,
            server_name: config.server_name,
        }
    }

    #[cfg_attr(
        not(feature = "notifications"),
        expect(
            unused_variables,
            reason = "the notification service is read from the extension state only when the \
                      runtime assembles it"
        )
    )]
    #[must_use]
    pub fn for_host(
        parts: &crate::HostParts,
        extension_state: &Dependencies,
        links: Arc<dyn LinkIssuer>,
        server_name: McpServerId,
    ) -> Self {
        Self::new(DocumentsMcpConfig {
            service: Arc::clone(parts.documents.service()),
            #[cfg(feature = "notifications")]
            notifications: extension_state
                .find::<NotificationsServiceSlot>()
                .and_then(|slot| slot.get().cloned()),
            uploads: Some(Arc::clone(parts.documents.uploads())),
            users: Arc::clone(&parts.user_repository),
            links,
            contexts: Arc::clone(parts.documents.contexts()),
            server_name,
        })
    }

    #[must_use]
    pub fn service(&self) -> &DocumentService {
        &self.service
    }

    #[cfg(feature = "notifications")]
    #[must_use]
    pub fn notifications(&self) -> Option<&NotificationsService> {
        self.notifications.as_deref()
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
    pub fn link_issuer(&self) -> &dyn LinkIssuer {
        self.links.as_ref()
    }

    #[must_use]
    pub const fn contexts(&self) -> &DynContextDirectory {
        &self.contexts
    }

    #[must_use]
    pub const fn server_name(&self) -> &McpServerId {
        &self.server_name
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
}
