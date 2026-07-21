//! MCP protocol metadata helpers.
//!
//! Non-wire MCP support types: server capabilities and UI/CSP config,
//! deployment descriptors, the registry and tool/deployment provider
//! traits (with `dyn`-compatible aliases), server lifecycle state, and
//! tool-result metadata extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod apps;
mod call_tool_result_ext;
pub mod capabilities;
pub mod deployment;
pub mod registry;
pub mod registry_trait;
pub mod server;
mod tool_result_metadata;

pub use apps::{
    EXTENSION_ID, LATEST_PROTOCOL_VERSION, McpUiToolMeta, RESOURCE_MIME_TYPE, SizeChangedParams,
    UI_META_KEY, UiInitializeParams, UiMessageParams, UiMessageRole, UiMethod,
    ui_method_js_constants,
};
pub use call_tool_result_ext::CallToolResultExt;
pub use capabilities::{
    MCP_APP_MIME_TYPE, McpAppsUiConfig, McpCspDomains, McpCspDomainsBuilder, McpExtensionId,
    McpResourceUiMeta, ToolVisibility, default_visibility, model_only_visibility,
    visibility_to_json,
};
pub use deployment::{
    Deployment, DeploymentConfig, ExternalAuth, McpServerType, OAuthRequirement, Settings,
};
pub use registry::RegistryConfig;
pub use registry_trait::{
    DynMcpDeploymentProvider, DynMcpRegistry, DynMcpToolProvider, McpDeploymentProvider,
    McpProvider, McpRegistry, McpServerState, McpToolProvider,
};
pub use server::{ERROR, McpAuthState, McpServerConfig, RUNNING, STARTING, STOPPED};
pub use tool_result_metadata::McpToolResultMetadata;
