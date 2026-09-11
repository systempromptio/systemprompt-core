//! Claude Desktop managed-policy construction and platform rendering.
//!
//! `claude_desktop_policy` defines the keys; registry and plist renderers
//! serialize them for the target platform.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use crate::install::xml;

/// A managed-policy value in the shape the policy declares, before any
/// platform's encoding is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValue {
    Str(String),
    Bool(bool),
    Json(serde_json::Value),
}

pub type PolicyEntry = (&'static str, PolicyValue);

/// One managed MCP server as the policy publishes it. `tool_policy` is
/// Claude Desktop's per-tool decision (tool name → `allow` / `ask` /
/// `blocked`); empty leaves the app's own default (ask).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerEntry {
    pub name: String,
    pub url: String,
    pub bearer: String,
    pub tool_policy: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct PolicyInputs<'a> {
    pub base_url: &'a str,
    pub api_key: &'a str,
    pub models: Option<String>,
    pub headers: &'a BTreeMap<String, String>,
    pub egress_allowed_hosts: Option<&'a [String]>,
    pub org_uuid: Option<&'a str>,
    pub mcp_servers: &'a [McpServerEntry],
}

#[must_use]
pub fn claude_desktop_policy(inputs: &PolicyInputs<'_>) -> Vec<PolicyEntry> {
    let mut out = inference_entries(inputs);
    out.extend(hardening_entries());
    if let Some(hosts) = super::cowork_egress_allowed_hosts(inputs.egress_allowed_hosts) {
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
    if let Some(uuid) = inputs.org_uuid.filter(|u| super::is_uuid_like(u)) {
        out.push((
            "deploymentOrganizationUuid",
            PolicyValue::Str(uuid.to_owned()),
        ));
    }
    out.push(("managedMcpServers", mcp_value(inputs.mcp_servers)));
    out
}

// Why: Claude Desktop breaks when `inferenceModels` names a non-Anthropic
// family, so every list is filtered to Claude ids at the one place the key is
// built. Desktop-only by construction (all callers go through
// `claude_desktop_policy`); it is the single carve-out from the reachability
// rule — every other host gets the whole advertised catalog, since the gateway
// transcodes every inbound wire to every provider wire.
fn anthropic_only(models: &serde_json::Value) -> serde_json::Value {
    let Some(arr) = models.as_array() else {
        return json_of(&super::default_inference_models());
    };
    let kept: Vec<serde_json::Value> = arr
        .iter()
        .filter(|m| {
            m.as_str().is_some_and(|id| {
                let lower = id.to_ascii_lowercase();
                lower.contains("claude") || lower.contains("anthropic")
            })
        })
        .cloned()
        .collect();
    if kept.is_empty() {
        json_of(&super::default_inference_models())
    } else {
        serde_json::Value::Array(kept)
    }
}

fn inference_entries(inputs: &PolicyInputs<'_>) -> Vec<PolicyEntry> {
    let models = anthropic_only(
        &inputs
            .models
            .as_deref()
            .filter(|m| !m.trim().is_empty())
            .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
            .unwrap_or_else(|| json_of(&super::default_inference_models())),
    );
    vec![
        ("inferenceProvider", PolicyValue::Str("gateway".into())),
        (
            "inferenceGatewayBaseUrl",
            PolicyValue::Str(inputs.base_url.to_owned()),
        ),
        (
            "inferenceGatewayApiKey",
            PolicyValue::Str(inputs.api_key.to_owned()),
        ),
        (
            "inferenceGatewayAuthScheme",
            PolicyValue::Str("bearer".into()),
        ),
        ("inferenceModels", PolicyValue::Json(models)),
    ]
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

fn mcp_value(servers: &[McpServerEntry]) -> PolicyValue {
    PolicyValue::Json(serde_json::Value::Array(
        servers
            .iter()
            .map(|s| {
                let mut entry = serde_json::json!({
                    "name": s.name,
                    "url": s.url,
                    "transport": "http",
                    "headers": { "Authorization": s.bearer },
                });
                if !s.tool_policy.is_empty() {
                    entry["toolPolicy"] = json_of(&s.tool_policy);
                }
                entry
            })
            .collect(),
    ))
}

fn json_of<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

#[cfg(target_os = "windows")]
#[must_use]
pub fn reg_values(policy: &[PolicyEntry]) -> Vec<(&'static str, &'static str, String)> {
    policy
        .iter()
        .map(|(name, value)| (*name, "REG_SZ", reg_encode(value)))
        .collect()
}

// Why: Claude's registry encoding requires strings, with arrays and objects
// encoded as JSON text.
#[cfg(target_os = "windows")]
fn reg_encode(value: &PolicyValue) -> String {
    match value {
        PolicyValue::Str(s) => s.clone(),
        PolicyValue::Bool(b) => b.to_string(),
        PolicyValue::Json(v) => v.to_string(),
    }
}

#[must_use]
pub fn plist_body(policy: &[PolicyEntry], indent: &str) -> String {
    let mut out = String::new();
    for (name, value) in policy {
        out.push_str(&format!("{indent}<key>{}</key>\n", xml::escape(name)));
        out.push_str(&plist_value(value, indent));
    }
    out
}

// Why: Claude's published preference encoding specifies string booleans, not
// plist booleans.
fn plist_value(value: &PolicyValue, indent: &str) -> String {
    match value {
        PolicyValue::Str(s) => format!("{indent}<string>{}</string>\n", xml::escape(s)),
        PolicyValue::Bool(b) => format!("{indent}<string>{b}</string>\n"),
        PolicyValue::Json(v) => plist_json(v, indent),
    }
}

fn plist_json(value: &serde_json::Value, indent: &str) -> String {
    let inner = format!("{indent}  ");
    match value {
        serde_json::Value::Null => format!("{indent}<string></string>\n"),
        serde_json::Value::Bool(b) => format!("{indent}<string>{b}</string>\n"),
        serde_json::Value::Number(n) => format!("{indent}<string>{n}</string>\n"),
        serde_json::Value::String(s) => {
            format!("{indent}<string>{}</string>\n", xml::escape(s))
        },
        serde_json::Value::Array(items) => {
            let mut out = format!("{indent}<array>\n");
            for item in items {
                out.push_str(&plist_json(item, &inner));
            }
            out.push_str(&format!("{indent}</array>\n"));
            out
        },
        serde_json::Value::Object(map) => {
            let mut out = format!("{indent}<dict>\n");
            for (k, v) in map {
                out.push_str(&format!("{inner}<key>{}</key>\n", xml::escape(k)));
                out.push_str(&plist_json(v, &inner));
            }
            out.push_str(&format!("{indent}</dict>\n"));
            out
        },
    }
}

pub fn mcp_entries(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
) -> std::io::Result<Vec<McpServerEntry>> {
    if registry.is_empty() {
        return Ok(Vec::new());
    }
    let bearer = loopback.bearer()?;
    let catalog = super::tool_catalog::read()?;
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    // Why: Desktop's `toolPolicy` names tools one by one, so a wildcard deny
    // can only be expressed over names the catalog knows; a server denied
    // outright is withheld from the policy instead of prompting for tools
    // the catalog has not seen.
    Ok(slugs
        .into_iter()
        .filter(|slug| {
            registry
                .get(*slug)
                .is_none_or(|upstream| !super::desktop_tool_policy::denied_outright(upstream))
        })
        .map(|slug| McpServerEntry {
            name: slug.clone(),
            url: loopback.mcp_url(slug.as_str()),
            bearer: bearer.clone(),
            tool_policy: registry.get(slug).map_or_else(BTreeMap::new, |upstream| {
                super::desktop_tool_policy::desktop_tool_policy_map(
                    upstream,
                    catalog.get(slug).map_or(&[][..], Vec::as_slice),
                )
            }),
        })
        .collect())
}
