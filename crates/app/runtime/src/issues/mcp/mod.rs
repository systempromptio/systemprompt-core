//! [`IssuesMcpTools`]: the issue report tools an install's MCP server
//! serves.
//!
//! `report_issue` files a report and records the request's own trace,
//! context, session, task and MCP execution, so triage follows the report to
//! what happened without the agent copying a log line into it;
//! `issue_attachment` declares an attachment upload (signed links the client
//! sends the bytes to out of band) and links a completed one; `issues_list`
//! and `issue_status` read the caller's own reports. A refusal is a tool
//! result with `isError: true` whose `structuredContent` carries a `code`.
//! The server that composes these tools authenticates each call first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod attach;
pub mod read;
pub mod refusal;
pub mod report;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use systemprompt_files::issues::IssueService;
use systemprompt_files::uploads::UploadService;
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::{ClientProfile, McpToolExecutor, McpToolHandler};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::execution::context::RequestContext;

use self::attach::AttachmentHandler;
use self::read::{ListHandler, StatusHandler};
use self::refusal::refusal_result;
use self::report::ReportHandler;
use crate::AppContext;

pub use self::refusal::IssueRefusalCode;

pub const INSTRUCTIONS: &str = "Issue reports. File a bug or feedback report with report_issue \
when a tool keeps failing, returns something wrong, or refuses where it should not; report a \
problem once, not on every retry: a repeat of an open report folds into it. Describe what you \
did, what you expected and what happened; the call's trace and context are recorded for you, so \
never paste logs, tokens or secrets. Attach files (a zip of diagnostics, a screenshot) with \
issue_attachment: declare, send the bytes to the returned links, then attach. Follow your reports \
with issues_list and issue_status.";

const TOOL_NAMES: [&str; 4] = [
    "report_issue",
    "issue_attachment",
    "issues_list",
    "issue_status",
];

/// Why [`IssuesMcpTools::from_context`] refused to build the tools.
#[derive(Debug, thiserror::Error)]
pub enum IssuesMcpError {
    #[error(
        "MCP server {server_name} serves issue report tools but issue reports are disabled: set \
         files.issues.enabled: true in services/config/files.yaml"
    )]
    Disabled { server_name: McpServerId },
}

/// One tool call, already authenticated by the composing server.
#[derive(Debug, Clone, Copy)]
pub struct IssuesToolCall<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub context: &'a RequestContext,
    pub client: &'a ClientProfile,
}

/// The four issue report tools of one MCP server.
#[derive(Clone)]
pub struct IssuesMcpTools {
    issues: Arc<IssueService>,
    uploads: Option<Arc<UploadService>>,
    tokens: Arc<dyn TokenAuthority>,
    server_name: McpServerId,
    api_external_url: String,
}

impl std::fmt::Debug for IssuesMcpTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuesMcpTools")
            .field("server_name", &self.server_name)
            .field("api_external_url", &self.api_external_url)
            .finish_non_exhaustive()
    }
}

impl IssuesMcpTools {
    #[must_use]
    pub fn new(
        issues: Arc<IssueService>,
        uploads: Option<Arc<UploadService>>,
        tokens: Arc<dyn TokenAuthority>,
        server_name: McpServerId,
        api_external_url: String,
    ) -> Self {
        Self {
            issues,
            uploads,
            tokens,
            server_name,
            api_external_url,
        }
    }

    pub fn from_context(
        ctx: &AppContext,
        server_name: McpServerId,
    ) -> Result<Self, IssuesMcpError> {
        if !ctx.files_config().issues_config().is_enabled() {
            return Err(IssuesMcpError::Disabled { server_name });
        }
        let uploads = ctx
            .files_config()
            .documents_config()
            .is_enabled()
            .then(|| Arc::clone(ctx.documents().uploads()));
        Ok(Self::new(
            Arc::clone(ctx.documents().issues()),
            uploads,
            ctx.token_authority(),
            server_name,
            ctx.config().api_external_url.clone(),
        ))
    }

    pub(super) fn issues(&self) -> &IssueService {
        &self.issues
    }

    pub(super) fn uploads(&self) -> Option<&UploadService> {
        self.uploads.as_deref()
    }

    pub(super) fn token_authority(&self) -> &dyn TokenAuthority {
        self.tokens.as_ref()
    }

    pub(super) fn api_external_url(&self) -> &str {
        &self.api_external_url
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
        vec![
            ReportHandler { tools: self }.tool_definition(&self.server_name),
            AttachmentHandler { tools: self }.tool_definition(&self.server_name),
            ListHandler { tools: self }.tool_definition(&self.server_name),
            StatusHandler { tools: self }.tool_definition(&self.server_name),
        ]
    }

    pub async fn call(&self, call: IssuesToolCall<'_>) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "report_issue" => run(call, &ReportHandler { tools: self }).await,
            "issue_attachment" => run(call, &AttachmentHandler { tools: self }).await,
            "issues_list" => run(call, &ListHandler { tools: self }).await,
            "issue_status" => run(call, &StatusHandler { tools: self }).await,
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

async fn run<H: McpToolHandler>(
    call: IssuesToolCall<'_>,
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
