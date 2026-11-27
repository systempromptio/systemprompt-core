//! The wire shape of a settings file, before it is checked and resolved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Compaction, Formatter, LspServer, McpServer};
use crate::config::error::FieldError;

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct RawSettings {
    pub(super) model: Option<String>,
    pub(super) small_model: Option<String>,
    pub(super) default_agent: Option<String>,
    // JSON: per-agent override objects, resolved by `AgentOverride::from_map`.
    pub(super) agent: IndexMap<String, Map<String, Value>>,
    pub(super) mcp: IndexMap<String, RawMcp>,
    pub(super) lsp: IndexMap<String, LspServer>,
    pub(super) formatter: IndexMap<String, Formatter>,
    pub(super) shell: Option<String>,
    pub(super) instructions: Vec<String>,
    pub(super) compaction: Compaction,
    // JSON: experimental switches are free-form values the engine reads by name.
    pub(super) experimental: IndexMap<String, Value>,
    // JSON: settings keys this crate does not interpret, kept as written.
    #[serde(flatten)]
    pub(super) extra: Map<String, Value>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(super) struct RawMcp {
    #[serde(rename = "type")]
    kind: Option<String>,
    url: Option<String>,
    headers: IndexMap<String, String>,
    command: Option<String>,
    args: Vec<String>,
    env: IndexMap<String, String>,
    enabled: Option<bool>,
    timeout_ms: Option<u64>,
}

impl RawMcp {
    pub(super) fn resolve(self) -> Result<McpServer, FieldError> {
        let enabled = self.enabled.unwrap_or(true);
        let kind = self
            .kind
            .clone()
            .or_else(|| self.url.as_ref().map(|_| "remote".to_owned()))
            .or_else(|| self.command.as_ref().map(|_| "local".to_owned()));
        match kind.as_deref() {
            Some("remote") => {
                let url = self
                    .url
                    .filter(|url| !url.trim().is_empty())
                    .ok_or(FieldError::Missing { field: "url" })?;
                Ok(McpServer::Remote {
                    url,
                    headers: self.headers,
                    enabled,
                    timeout_ms: self.timeout_ms,
                })
            },
            Some("local") => {
                let command = self
                    .command
                    .filter(|command| !command.trim().is_empty())
                    .ok_or(FieldError::Missing { field: "command" })?;
                Ok(McpServer::Local {
                    command,
                    args: self.args,
                    env: self.env,
                    enabled,
                    timeout_ms: self.timeout_ms,
                })
            },
            Some(other) => Err(FieldError::Value {
                value: other.to_owned(),
                expected: "remote or local",
            }),
            None => Err(FieldError::Missing { field: "type" }),
        }
    }
}
