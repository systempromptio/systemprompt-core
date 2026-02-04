use serde::{Deserialize, Serialize};
use std::fmt;

pub const MCP_APP_MIME_TYPE: &str = "text/html;profile=mcp-app";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum McpExtensionId {
    #[serde(rename = "io.modelcontextprotocol/ui")]
    McpAppsUi,
    #[serde(untagged)]
    Custom(String),
}

impl McpExtensionId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::McpAppsUi => "io.modelcontextprotocol/ui",
            Self::Custom(s) => s,
        }
    }

    pub fn custom(id: impl Into<String>) -> Self {
        Self::Custom(id.into())
    }
}

impl fmt::Display for McpExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAppsUiConfig {
    #[serde(default = "default_mime_types")]
    pub mime_types: Vec<String>,
}

impl Default for McpAppsUiConfig {
    fn default() -> Self {
        Self {
            mime_types: default_mime_types(),
        }
    }
}

impl McpAppsUiConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "mimeTypes": self.mime_types
        })
    }
}

fn default_mime_types() -> Vec<String> {
    vec![MCP_APP_MIME_TYPE.to_string()]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolVisibility {
    #[default]
    Model,
    App,
}

impl fmt::Display for ToolVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model => write!(f, "model"),
            Self::App => write!(f, "app"),
        }
    }
}

pub fn default_visibility() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model, ToolVisibility::App]
}

pub fn model_only_visibility() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model]
}

pub fn visibility_to_json(visibility: &[ToolVisibility]) -> serde_json::Value {
    serde_json::json!(visibility)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct McpCspDomains {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connect_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frame_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub base_uri_domains: Vec<String>,
}

impl McpCspDomains {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn builder() -> McpCspDomainsBuilder {
        McpCspDomainsBuilder::new()
    }
}

#[derive(Debug, Default)]
pub struct McpCspDomainsBuilder {
    inner: McpCspDomains,
}

impl McpCspDomainsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn connect_domain(mut self, domain: impl Into<String>) -> Self {
        self.inner.connect_domains.push(domain.into());
        self
    }

    pub fn connect_domains(mut self, domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner
            .connect_domains
            .extend(domains.into_iter().map(Into::into));
        self
    }

    pub fn resource_domain(mut self, domain: impl Into<String>) -> Self {
        self.inner.resource_domains.push(domain.into());
        self
    }

    pub fn resource_domains(
        mut self,
        domains: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.inner
            .resource_domains
            .extend(domains.into_iter().map(Into::into));
        self
    }

    pub fn frame_domain(mut self, domain: impl Into<String>) -> Self {
        self.inner.frame_domains.push(domain.into());
        self
    }

    pub fn base_uri_domain(mut self, domain: impl Into<String>) -> Self {
        self.inner.base_uri_domains.push(domain.into());
        self
    }

    pub fn build(self) -> McpCspDomains {
        self.inner
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceUiMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csp: Option<McpCspDomains>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prefers_border: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

impl McpResourceUiMeta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_csp(mut self, csp: McpCspDomains) -> Self {
        self.csp = Some(csp);
        self
    }

    pub fn with_csp_opt(mut self, csp: Option<McpCspDomains>) -> Self {
        self.csp = csp;
        self
    }

    pub const fn with_prefers_border(mut self, prefers: bool) -> Self {
        self.prefers_border = prefers;
        self
    }

    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({});
        if let Some(csp) = &self.csp {
            obj["csp"] = serde_json::json!(csp);
        }
        if self.prefers_border {
            obj["prefersBorder"] = serde_json::json!(true);
        }
        if let Some(domain) = &self.domain {
            obj["domain"] = serde_json::json!(domain);
        }
        obj
    }

    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        meta.insert("ui".to_string(), self.to_json());
        meta
    }
}
