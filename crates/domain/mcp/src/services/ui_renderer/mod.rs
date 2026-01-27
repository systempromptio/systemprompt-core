mod csp;
pub mod registry;
pub mod templates;

pub use csp::{CspBuilder, CspPolicy};
pub use registry::UiRendererRegistry;

use anyhow::Result;
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

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
    pub visibility: Vec<String>,
}

impl UiMetadata {
    pub fn for_artifact(artifact_id: &str, source: Option<&str>) -> Self {
        let server = source.unwrap_or("systemprompt");
        Self {
            resource_uri: format!("ui://{server}/{artifact_id}"),
            csp: None,
            visibility: vec!["model".to_string()],
        }
    }

    pub fn for_tool_definition(server_name: &str) -> Self {
        Self {
            resource_uri: format!("ui://{server_name}/{{artifact_id}}"),
            csp: None,
            visibility: vec!["model".to_string()],
        }
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
        meta.insert("ui".to_string(), self.to_json());
        meta
    }

    pub fn to_result_meta(&self, artifact_id: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        let ui_with_id = serde_json::json!({
            "resourceUri": self.resource_uri.replace("{artifact_id}", artifact_id),
            "visibility": self.visibility
        });
        meta.insert("ui".to_string(), ui_with_id);
        meta
    }
}

#[async_trait]
pub trait UiRenderer: Send + Sync {
    fn artifact_type(&self) -> ArtifactType;

    fn supports(&self, artifact_type: &str) -> bool {
        self.artifact_type().to_string() == artifact_type
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource>;

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}
