//! Rendering of A2A artifacts into MCP UI resources.
//!
//! The [`UiRenderer`] trait maps an [`ArtifactType`] to an HTML/text resource;
//! concrete renderers live in [`templates`] and are dispatched through
//! [`UiRendererRegistry`]. [`CspPolicy`] carries the content-security-policy
//! constraints attached to rendered resources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifact_ui;
mod csp;
pub mod registry;
pub mod templates;

pub use artifact_ui::{
    RenderTarget, artifact_resource_uri, artifact_ui_resource, parse_artifact_resource_uri,
};
pub use csp::{CspBuilder, CspPolicy};
pub use registry::UiRendererRegistry;

use crate::error::McpDomainResult;
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;
use systemprompt_models::mcp::{McpResourceUiMeta, ToolVisibility};

pub const MCP_APP_MIME_TYPE: &str = "text/html;profile=mcp-app";

#[derive(Debug, Clone)]
pub struct UiResource {
    pub html: String,
    pub csp: CspPolicy,
}

impl UiResource {
    pub fn new(html: String) -> Self {
        Self {
            html,
            csp: CspPolicy::default(),
        }
    }

    pub fn with_csp(mut self, csp: CspPolicy) -> Self {
        self.csp = csp;
        self
    }

    pub const fn mime_type() -> &'static str {
        MCP_APP_MIME_TYPE
    }
}

#[derive(Debug, Clone)]
pub struct UiMetadata {
    pub resource_uri: String,
    pub csp: Option<CspPolicy>,
    pub visibility: Vec<ToolVisibility>,
    pub prefers_border: bool,
}

impl UiMetadata {
    pub fn for_static_template(server_name: &str) -> Self {
        Self {
            resource_uri: format!("ui://{server_name}/artifact-viewer"),
            csp: None,
            visibility: vec![ToolVisibility::Model, ToolVisibility::App],
            prefers_border: true,
        }
    }

    pub fn for_tool_definition(server_name: &str) -> Self {
        Self {
            resource_uri: format!("ui://{server_name}/artifact-viewer"),
            csp: None,
            visibility: vec![ToolVisibility::Model, ToolVisibility::App],
            prefers_border: true,
        }
    }

    pub fn with_csp(mut self, csp: CspPolicy) -> Self {
        self.csp = Some(csp);
        self
    }

    pub const fn with_prefers_border(mut self, prefers: bool) -> Self {
        self.prefers_border = prefers;
        self
    }

    pub fn model_only(mut self) -> Self {
        self.visibility = vec![ToolVisibility::Model];
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut meta = serde_json::json!({
            "resourceUri": self.resource_uri,
            "visibility": self.visibility
        });

        if let Some(csp) = &self.csp {
            meta["csp"] = serde_json::json!(csp.to_header_value());
        }

        meta
    }

    pub fn to_tool_meta(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        meta.insert("ui".to_owned(), self.to_json());
        meta
    }

    pub fn to_resource_meta(&self) -> McpResourceUiMeta {
        let csp_domains = self.csp.as_ref().map(CspPolicy::to_mcp_domains);
        McpResourceUiMeta::new()
            .with_prefers_border(self.prefers_border)
            .with_csp_opt(csp_domains)
    }
}

// `#[async_trait]` required: renderers are stored and dispatched as `Arc<dyn
// UiRenderer>` in `UiRendererRegistry`, so the trait must stay
// `dyn`-compatible.
#[async_trait]
pub trait UiRenderer: Send + Sync {
    fn artifact_type(&self) -> ArtifactType;

    fn supports(&self, artifact_type: &str) -> bool {
        self.artifact_type().to_string() == artifact_type
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource>;

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}
