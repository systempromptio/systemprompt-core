//! Typed bindings for the MCP Apps extension (SEP-1865, `2026-01-26`).
//!
//! Everything the extension puts on the wire is modelled here so no call site
//! hand-writes a method name or a `_meta` key: [`UiMethod`] enumerates the
//! app-to-host protocol, [`McpUiToolMeta`] and
//! [`McpResourceUiMeta`](super::McpResourceUiMeta) carry the metadata, and the
//! `*Params` types carry the message bodies. Field names and string literals
//! track the normative schema at
//! <https://github.com/modelcontextprotocol/ext-apps>.
//!
//! Two shapes are easy to get wrong and are pinned by wire-shape tests:
//! [`UiMessageParams::content`] is an array of content blocks rather than a
//! bare block, and [`McpUiToolMeta`] omits `csp` and `permissions` because the
//! schema types them as `never` on tools — they belong on the UI resource.
//! [`ui_method_js_constants`] projects [`UiMethod`] into the browser so app
//! templates cannot drift from this enum.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::capabilities::ToolVisibility;
use rmcp::model::{ContentBlock, Implementation};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const EXTENSION_ID: &str = "io.modelcontextprotocol/ui";

pub const RESOURCE_MIME_TYPE: &str = "text/html;profile=mcp-app";

pub const LATEST_PROTOCOL_VERSION: &str = "2026-01-26";

pub const UI_META_KEY: &str = "ui";

/// Emitted alongside `_meta.ui` for hosts predating it, as the reference SDK
/// does.
pub const LEGACY_RESOURCE_URI_META_KEY: &str = "ui/resourceUri";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UiMethod {
    #[serde(rename = "ui/initialize")]
    Initialize,
    #[serde(rename = "ui/notifications/initialized")]
    Initialized,
    #[serde(rename = "ui/notifications/tool-input")]
    ToolInput,
    #[serde(rename = "ui/notifications/tool-input-partial")]
    ToolInputPartial,
    #[serde(rename = "ui/notifications/tool-result")]
    ToolResult,
    #[serde(rename = "ui/notifications/tool-cancelled")]
    ToolCancelled,
    #[serde(rename = "ui/notifications/size-changed")]
    SizeChanged,
    #[serde(rename = "ui/notifications/host-context-changed")]
    HostContextChanged,
    #[serde(rename = "ui/request-display-mode")]
    RequestDisplayMode,
    #[serde(rename = "ui/message")]
    Message,
    #[serde(rename = "ui/update-model-context")]
    UpdateModelContext,
    #[serde(rename = "ui/open-link")]
    OpenLink,
    #[serde(rename = "ui/download-file")]
    DownloadFile,
    #[serde(rename = "ui/resource-teardown")]
    ResourceTeardown,
    #[serde(rename = "ui/notifications/request-teardown")]
    RequestTeardown,
}

impl UiMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialize => "ui/initialize",
            Self::Initialized => "ui/notifications/initialized",
            Self::ToolInput => "ui/notifications/tool-input",
            Self::ToolInputPartial => "ui/notifications/tool-input-partial",
            Self::ToolResult => "ui/notifications/tool-result",
            Self::ToolCancelled => "ui/notifications/tool-cancelled",
            Self::SizeChanged => "ui/notifications/size-changed",
            Self::HostContextChanged => "ui/notifications/host-context-changed",
            Self::RequestDisplayMode => "ui/request-display-mode",
            Self::Message => "ui/message",
            Self::UpdateModelContext => "ui/update-model-context",
            Self::OpenLink => "ui/open-link",
            Self::DownloadFile => "ui/download-file",
            Self::ResourceTeardown => "ui/resource-teardown",
            Self::RequestTeardown => "ui/notifications/request-teardown",
        }
    }

    #[must_use]
    pub const fn js_const(self) -> &'static str {
        match self {
            Self::Initialize => "INITIALIZE",
            Self::Initialized => "INITIALIZED",
            Self::ToolInput => "TOOL_INPUT",
            Self::ToolInputPartial => "TOOL_INPUT_PARTIAL",
            Self::ToolResult => "TOOL_RESULT",
            Self::ToolCancelled => "TOOL_CANCELLED",
            Self::SizeChanged => "SIZE_CHANGED",
            Self::HostContextChanged => "HOST_CONTEXT_CHANGED",
            Self::RequestDisplayMode => "REQUEST_DISPLAY_MODE",
            Self::Message => "MESSAGE",
            Self::UpdateModelContext => "UPDATE_MODEL_CONTEXT",
            Self::OpenLink => "OPEN_LINK",
            Self::DownloadFile => "DOWNLOAD_FILE",
            Self::ResourceTeardown => "RESOURCE_TEARDOWN",
            Self::RequestTeardown => "REQUEST_TEARDOWN",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Initialize,
            Self::Initialized,
            Self::ToolInput,
            Self::ToolInputPartial,
            Self::ToolResult,
            Self::ToolCancelled,
            Self::SizeChanged,
            Self::HostContextChanged,
            Self::RequestDisplayMode,
            Self::Message,
            Self::UpdateModelContext,
            Self::OpenLink,
            Self::DownloadFile,
            Self::ResourceTeardown,
            Self::RequestTeardown,
        ]
    }
}

impl fmt::Display for UiMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpUiToolMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Vec<ToolVisibility>>,
}

impl McpUiToolMeta {
    #[must_use]
    pub fn new(resource_uri: impl Into<String>) -> Self {
        Self {
            resource_uri: Some(resource_uri.into()),
            visibility: None,
        }
    }

    #[must_use]
    pub fn with_visibility(mut self, visibility: Vec<ToolVisibility>) -> Self {
        self.visibility = Some(visibility);
        self
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SizeChangedParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMessageParams {
    pub role: UiMessageRole,
    pub content: Vec<ContentBlock>,
}

impl UiMessageParams {
    #[must_use]
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: UiMessageRole::User,
            content: vec![ContentBlock::text(text.into())],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMessageRole {
    #[default]
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiInitializeParams {
    pub app_info: Implementation,
    // JSON: protocol boundary — McpUiAppCapabilities is an open object hosts
    // extend, so the schema fixes no field set to type against.
    pub app_capabilities: serde_json::Value,
    pub protocol_version: String,
}

impl UiInitializeParams {
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            app_info: Implementation::new(name.into(), version.into()),
            app_capabilities: serde_json::json!({}),
            protocol_version: LATEST_PROTOCOL_VERSION.to_owned(),
        }
    }
}

#[must_use]
pub fn ui_method_js_constants() -> String {
    let entries = UiMethod::all()
        .iter()
        .map(|m| format!("    {}: '{}'", m.js_const(), m.as_str()))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "const MCP_UI = Object.freeze({{\n{entries},\n    PROTOCOL_VERSION: \
         '{LATEST_PROTOCOL_VERSION}'\n}});\n"
    )
}
