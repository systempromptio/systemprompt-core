//! The Claude Desktop managed policy: one key set, rendered per platform.
//!
//! The policy Cowork reads is a single set of keys in the
//! `com.anthropic.claudefordesktop` domain, but it used to be written three
//! separate times — as registry values on Windows, and as hand-written plist
//! XML in both the `install --apply` templates and the GUI host profile. The
//! three drifted: only Windows carried `allowedWorkspaceFolders`, only the GUI
//! profile carried `inferenceModels`, and the macOS MCP block published the
//! upstream URL with no credential where Windows published the loopback proxy
//! and its bearer.
//!
//! [`claude_desktop_policy`] is now the only place the key set is decided.
//! `reg_values` and [`plist_body`] render it; a template supplies the
//! surrounding document and nothing else. Both renderers are platform-neutral,
//! so the macOS plist can be tested from any host — it previously could not be.
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

/// One managed MCP server as the policy publishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerEntry {
    pub name: String,
    pub url: String,
    pub bearer: String,
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

// Why: the order is the order the keys are written in, which keeps a rendered
// profile byte-stable across runs — `macos::apply` compares bytes to decide
// whether to raise an administrator prompt, so an unstable order would prompt
// on every sync.
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
    if let Some(entry) = workspace_entry() {
        out.push(entry);
    }
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

// Why: Cowork treats `inferenceProvider=gateway` without a base URL and a
// credential as unusable and refuses to start any task, so these are written as
// one unit or not at all. A model list already on the machine wins over the
// default, which is how a gateway-supplied list survives a re-apply.
fn inference_entries(inputs: &PolicyInputs<'_>) -> Vec<PolicyEntry> {
    let models = inputs
        .models
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
        .unwrap_or_else(|| json_of(&super::default_inference_models()));
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

// Why: `disableNonessentialServices` is written as an explicit `false` rather
// than omitted, because `true` blocks the claudemcpcontent.com renderer that
// MCP display extensions load from — an older `true` must be corrected on
// drift, which omitting the key would not do.
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

// Why: without a pre-trusted workspace Cowork falls back to protected host
// paths and blocks on `request_cowork_directory`; `isDefaultSelected` skips the
// trust prompt. This was a Windows-only key, which is why the macOS setup
// health row for it could never go green however the profile was installed.
fn workspace_entry() -> Option<PolicyEntry> {
    let workspace = crate::brand::brand().workspace_dir_name;
    (!workspace.is_empty()).then(|| {
        (
            "allowedWorkspaceFolders",
            PolicyValue::Json(
                serde_json::json!([{ "path": format!("~/{workspace}"), "isDefaultSelected": true }]),
            ),
        )
    })
}

// Why: servers point at the loopback proxy, never the gateway — the proxy is
// what stamps the per-user JWT and applies governance, and Cowork's OAuth flow
// rejects the gateway's non-HTTPS authorize URL besides. An empty list is
// written rather than omitted so a stale server clears.
fn mcp_value(servers: &[McpServerEntry]) -> PolicyValue {
    PolicyValue::Json(serde_json::Value::Array(
        servers
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "url": s.url,
                    "transport": "http",
                    "headers": { "Authorization": s.bearer },
                })
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

// Why: the published reference asks for every value as a string in an OS
// preference store, with arrays and objects as JSON text.
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

// Why: booleans are written as `<string>true</string>` rather than `<true/>`.
// Both are read, and the string form is what the published encoding table asks
// for; it also keeps one encoding across the plist and the registry.
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

// Why: every entry points at the loopback proxy, so a caller cannot
// accidentally publish the upstream gateway URL — the shape that could not
// authenticate.
pub fn mcp_entries(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
) -> std::io::Result<Vec<McpServerEntry>> {
    if registry.is_empty() {
        return Ok(Vec::new());
    }
    let bearer = loopback.bearer()?;
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    Ok(slugs
        .into_iter()
        .map(|slug| McpServerEntry {
            name: slug.clone(),
            url: loopback.mcp_url(slug.as_str()),
            bearer: bearer.clone(),
        })
        .collect())
}
