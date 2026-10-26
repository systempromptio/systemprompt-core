//! The `OpenCode` V2 `opencode.json` shape, rendered by one builder.
//!
//! Both the bridge (user or managed tier on a device) and the headless
//! `agent-runs` host (an isolated run home) emit this document, so the keys
//! cannot drift between them. Only the V2 shape (`@opencode/cli` >= 2.0.30)
//! is modelled: `providers.<id>{package, settings, models}`, `policies`,
//! `mcp.servers`.
//!
//! `policies` are evaluated last-match-wins by `OpenCode`; a `tool.use`
//! resource is `<tool>:<pattern>` and an `integration.use` resource is
//! `mcp:<name>`, `plugin:<id>` or `skill:<id>`. [`lockdown_policies`] relies
//! on that ordering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod policy;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use policy::{Lockdown, Policy, PolicyAction, PolicyEffect, lockdown_policies};

pub const SCHEMA_URL: &str = "https://opencode.ai/config.json";

pub const OPENAI_COMPATIBLE_PACKAGE: &str = "aisdk:@ai-sdk/openai-compatible";

pub const MIN_CLIENT_VERSION: semver::Version = semver::Version::new(2, 0, 30);

/// A whole V2 `opencode.json` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenCodeConfig {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, Provider>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policies: Vec<Policy>,
    #[serde(default, skip_serializing_if = "McpBlock::is_empty")]
    pub mcp: McpBlock,
}

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            schema: SCHEMA_URL.to_owned(),
            model: None,
            update: Some("disable".to_owned()),
            share: Some("disabled".to_owned()),
            providers: BTreeMap::new(),
            policies: Vec::new(),
            mcp: McpBlock::default(),
        }
    }
}

impl OpenCodeConfig {
    #[must_use]
    pub fn with_provider(mut self, id: &str, provider: Provider) -> Self {
        self.providers.insert(id.to_owned(), provider);
        self
    }

    #[must_use]
    pub fn with_model(mut self, provider: &str, alias: &str) -> Self {
        self.model = Some(format!("{provider}/{alias}"));
        self
    }

    #[must_use]
    pub fn with_policies(mut self, policies: Vec<Policy>) -> Self {
        self.policies = policies;
        self
    }

    #[must_use]
    pub fn with_mcp_server(mut self, name: &str, server: McpServer) -> Self {
        self.mcp.servers.insert(name.to_owned(), server);
        self
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|mut text| {
            text.push('\n');
            text
        })
    }
}

/// One `providers.<id>` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub package: String,
    pub settings: ProviderSettings,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, ModelEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSettings {
    #[serde(rename = "baseURL")]
    pub base_url: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntry {
    #[serde(rename = "modelID")]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<ModelLimit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelLimit {
    pub context: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpBlock {
    #[serde(default)]
    pub servers: BTreeMap<String, McpServer>,
}

impl McpBlock {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
}

/// One `mcp.servers.<name>` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServer {
    Remote {
        url: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        codemode: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        oauth: Option<McpOAuth>,
    },
    Local {
        command: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        environment: BTreeMap<String, String>,
    },
}

/// A remote server's `oauth` setting. `OpenCode` registers every remote server
/// as an OAuth integration and answers a 401 with client registration unless
/// `oauth` is `false`; only that opt-out is modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpOAuth {
    Disabled,
}

impl Serialize for McpOAuth {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(false)
    }
}

impl<'de> Deserialize<'de> for McpOAuth {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if bool::deserialize(deserializer)? {
            return Err(serde::de::Error::custom(
                "only `oauth: false` is modelled for a remote MCP server",
            ));
        }
        Ok(Self::Disabled)
    }
}

/// Where `OpenCode` reads a secret from at load time; never the secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyRef {
    Env(String),
    File(String),
}

impl ApiKeyRef {
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Env(var) => format!("{{env:{var}}}"),
            Self::File(path) => format!("{{file:{path}}}"),
        }
    }
}

#[must_use]
pub fn gateway_provider(
    base_url: &str,
    api_key: &ApiKeyRef,
    headers: BTreeMap<String, String>,
    models: BTreeMap<String, ModelEntry>,
) -> Provider {
    Provider {
        name: "systemprompt".to_owned(),
        package: OPENAI_COMPATIBLE_PACKAGE.to_owned(),
        settings: ProviderSettings {
            base_url: format!("{}/v1", base_url.trim_end_matches('/')),
            api_key: api_key.render(),
            headers,
        },
        models,
    }
}
