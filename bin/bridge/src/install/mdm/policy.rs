//! Claude Desktop managed-policy construction and platform rendering.
//!
//! `claude_desktop_policy` defines the keys; registry and plist renderers
//! serialize them for the target platform. The policy is readable by every
//! local account, so it carries the Claude Desktop host token derived from
//! the loopback secret — never the secret itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::bridge::host::HostKind;

use super::error::MdmError;
use crate::ids::{HostToken, LoopbackSecret};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValue {
    Str(String),
    Bool(bool),
    // JSON: Claude Desktop managed policy — `object[]` keys hold schema-checked JSON.
    Json(serde_json::Value),
}

pub type PolicyEntry = (&'static str, PolicyValue);

pub const WRITTEN_POLICY_KEYS: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceModels",
    "disableEssentialTelemetry",
    "disableNonessentialTelemetry",
    "disableNonessentialServices",
    "disableAutoUpdates",
    "disableDeploymentModeChooser",
    "isLocalDevMcpEnabled",
    "coworkEgressAllowedHosts",
    "allowedWorkspaceFolders",
    "inferenceCustomHeaders",
    "deploymentOrganizationUuid",
    "managedMcpServers",
];

/// One managed MCP server as the policy publishes it. `tool_policy` is
/// Claude Desktop's per-tool decision (tool name → `allow` / `ask` /
/// `blocked`); empty leaves the app's own default (ask).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerEntry {
    pub name: String,
    pub url: String,
    pub tool_policy: BTreeMap<String, String>,
}

/// `mcp_servers` is `None` when the connector list could not be projected
/// completely; the `managedMcpServers` key is then withheld rather than
/// written partially.
#[derive(Debug)]
pub struct PolicyInputs<'a> {
    pub base_url: &'a str,
    pub host_token: &'a HostToken,
    pub models: Option<String>,
    pub headers: &'a BTreeMap<String, String>,
    pub egress_allowed_hosts: Option<&'a [String]>,
    pub org_uuid: Option<&'a str>,
    pub mcp_servers: Option<&'a [McpServerEntry]>,
}

#[must_use]
pub fn desktop_host_token(secret: &LoopbackSecret) -> HostToken {
    crate::proxy::scoped_token::host_token(secret, HostKind::ClaudeDesktop)
}

pub fn claude_desktop_policy(inputs: &PolicyInputs<'_>) -> Result<Vec<PolicyEntry>, MdmError> {
    let mut out = super::inference::inference_entries(inputs)?;
    out.extend(hardening_entries());
    if let Some(hosts) = super::cowork_egress_allowed_hosts(inputs.egress_allowed_hosts)? {
        out.push((
            "coworkEgressAllowedHosts",
            PolicyValue::Json(json_of(&hosts)),
        ));
    }
    out.push(workspace_entry());
    if !inputs.headers.is_empty() {
        out.push((
            "inferenceCustomHeaders",
            PolicyValue::Json(json_of(inputs.headers)),
        ));
    }
    if let Some(uuid) = inputs.org_uuid {
        if !super::is_uuid_like(uuid) {
            return Err(MdmError::InvalidConfig {
                key: "deploymentOrganizationUuid",
                detail: format!("{uuid:?} is not a hyphenated UUID"),
            });
        }
        out.push((
            "deploymentOrganizationUuid",
            PolicyValue::Str(uuid.to_owned()),
        ));
    }
    if let Some(servers) = inputs.mcp_servers {
        out.push(("managedMcpServers", mcp_value(servers, inputs.host_token)));
    }
    Ok(out)
}

// Why: Cowork's disableNonessentialServices=true blocks the
// claudemcpcontent.com MCP renderer.
fn hardening_entries() -> Vec<PolicyEntry> {
    vec![
        ("disableEssentialTelemetry", PolicyValue::Bool(true)),
        ("disableNonessentialTelemetry", PolicyValue::Bool(true)),
        ("disableNonessentialServices", PolicyValue::Bool(false)),
        ("disableAutoUpdates", PolicyValue::Bool(true)),
        ("disableDeploymentModeChooser", PolicyValue::Bool(true)),
        ("isLocalDevMcpEnabled", PolicyValue::Bool(false)),
    ]
}

// Why: Cowork's isDefaultSelected pre-trusts the workspace and avoids
// request_cowork_directory, but the Claude Desktop Code tab enforces the
// same list as the only permitted workspace roots, so the home directory
// must be listed too or every folder outside the brand workspace is refused.
fn workspace_entry() -> PolicyEntry {
    (
        "allowedWorkspaceFolders",
        PolicyValue::Json(workspace_folders()),
    )
}

// JSON: Claude Desktop managed policy — `object[]` keys hold schema-checked
// JSON.
#[must_use]
pub fn workspace_folders() -> serde_json::Value {
    let workspace = crate::brand::brand().workspace_dir_name;
    let mut folders = Vec::new();
    if !workspace.is_empty() {
        folders.push(
            serde_json::json!({ "path": format!("~/{workspace}"), "isDefaultSelected": true }),
        );
    }
    folders.push(serde_json::json!({ "path": "~", "isDefaultSelected": false }));
    serde_json::Value::Array(folders)
}

fn mcp_value(servers: &[McpServerEntry], host_token: &HostToken) -> PolicyValue {
    let bearer = format!("Bearer {}", host_token.as_str());
    PolicyValue::Json(serde_json::Value::Array(
        servers
            .iter()
            .map(|s| {
                let mut entry = serde_json::json!({
                    "name": s.name,
                    "url": s.url,
                    "transport": "http",
                    "headers": { "Authorization": bearer },
                });
                if !s.tool_policy.is_empty() {
                    entry["toolPolicy"] = json_of(&s.tool_policy);
                }
                entry
            })
            .collect(),
    ))
}

// JSON: Claude Desktop managed policy — `object[]` keys hold schema-checked
// JSON.
pub(super) fn json_of<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

pub use super::policy_render::{plist_body, reg_values};

// Why: Desktop's `toolPolicy` names tools one by one, so a wildcard can only
// be expressed over names the catalog knows. A server denied outright or one
// the manifest gave no tool policy is withheld from the list; a wildcard the
// catalog cannot expand withholds the whole list (`None`), because a partial
// map is one Desktop would resolve to its own default.
pub fn mcp_entries(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
) -> std::io::Result<Option<Vec<McpServerEntry>>> {
    if registry.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let catalog = super::tool_catalog::read()?;
    Ok(mcp_entries_with(loopback, registry, &catalog))
}

pub fn mcp_entries_with(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
    catalog: &super::tool_catalog::ToolCatalog,
) -> Option<Vec<McpServerEntry>> {
    if registry.is_empty() {
        return Some(Vec::new());
    }
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    let mut out = Vec::with_capacity(slugs.len());
    for slug in slugs {
        let Some(upstream) = registry.get(slug) else {
            continue;
        };
        if upstream.tool_policy.is_empty() {
            tracing::warn!(target: "bridge::mdm", slug = %slug, "managed MCP server has no tool policy; withheld from the desktop policy");
            continue;
        }
        if super::desktop_tool_policy::denied_outright(upstream) {
            continue;
        }
        let Some(tool_policy) = super::desktop_tool_policy::desktop_tool_policy_map(
            upstream,
            catalog.get(slug).map_or(&[][..], Vec::as_slice),
        ) else {
            tracing::warn!(target: "bridge::mdm", slug = %slug, "tool catalog has no names for a wildcard tool policy; managedMcpServers withheld");
            return None;
        };
        out.push(McpServerEntry {
            name: slug.clone(),
            url: loopback.mcp_url(slug.as_str()),
            tool_policy,
        });
    }
    Some(out)
}
