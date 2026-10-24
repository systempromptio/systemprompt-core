//! [`RunRequest`]: everything a host needs to run one headless turn.
//!
//! The request is stored verbatim as `agent_runs.request` (JSONB), so it is
//! the contract between the product that queues a run and the executor that
//! claims it. [`HostKind`] names the coding-agent host; its spelling is the
//! `agent_runs.host` column.
//!
//! `RunRequest::attribution_scopes` is header-only attribution: every gateway
//! call the run makes carries one `x-systemprompt-scope-<dimension>: <value>`
//! header per binding (`scope_headers`), so a trace or cost rollup can key on
//! it without the run's API key being bound to the value (`workflow=<run_id>`,
//! `project=<id>`, …).
//!
//! [`RunRequest::harness`] names a managed revision installed into the host's
//! config tree before spawn; [`RunRequest::traffic`] classes the run's
//! gateway traffic (`production` unless the caller asks for `evaluation`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{HostSessionId, ManagedResourceId, PluginId, ResourceRevisionId};
use systemprompt_models::attribution::ScopeBinding;
pub use systemprompt_models::attribution::TrafficClass;
pub use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::managed::AssetDigest;

use crate::macros::text_enum;

text_enum!(
    PermissionMode, UnknownPermissionMode {
        Default => "default",
        AcceptEdits => "acceptEdits",
        BypassPermissions => "bypassPermissions",
        Plan => "plan",
    }
);

/// The managed revision a run installs into its host before spawn.
/// `bundle_digest` is the digest of the whole revision closure; a bundle
/// with any other digest is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessRef {
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub bundle_digest: AssetDigest,
}

/// An MCP server handed to the host, keyed by its name in
/// [`RunRequest::mcp_servers`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServerSpec {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRequest {
    pub host: HostKind,
    pub prompt: String,
    #[serde(default)]
    pub system_prompt_append: Option<String>,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    pub permission_mode: PermissionMode,
    #[serde(default)]
    pub permission_prompts_none: bool,
    pub workspace: PathBuf,
    #[serde(default)]
    pub add_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, McpServerSpec>,
    // JSON: a JSON Schema the host must make its final answer satisfy; its
    // shape is whatever the caller's output contract is.
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub max_budget_usd: Option<f64>,
    pub wall_clock: Duration,
    #[serde(default)]
    pub resume_session_id: Option<HostSessionId>,
    #[serde(default = "first_attempt")]
    pub attempt: u32,
    #[serde(default)]
    pub attribution_scopes: Vec<ScopeBinding>,
    #[serde(default)]
    pub plugin_id: Option<PluginId>,
    #[serde(default)]
    pub plugin_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub harness: Option<HarnessRef>,
    #[serde(default)]
    pub traffic: TrafficClass,
}

impl RunRequest {
    #[must_use]
    pub fn scope_headers(&self) -> BTreeMap<String, String> {
        let mut headers: BTreeMap<String, String> = self
            .attribution_scopes
            .iter()
            .map(|binding| {
                (
                    format!("x-systemprompt-scope-{}", binding.dimension.as_str()),
                    binding.value.clone(),
                )
            })
            .collect();
        if self.traffic == TrafficClass::Evaluation {
            headers.insert(
                format!("x-systemprompt-scope-{}", TrafficClass::DIMENSION),
                self.traffic.as_str().to_owned(),
            );
        }
        headers
    }

    #[must_use]
    pub fn traffic_binding(&self) -> Option<ScopeBinding> {
        (self.traffic == TrafficClass::Evaluation).then(|| ScopeBinding::traffic(self.traffic))
    }
}

const fn first_attempt() -> u32 {
    1
}
