//! The run home's `~/.config/opencode/opencode.json`.
//!
//! Rendered with the shared [`OpenCodeConfig`] builder: the gateway as the
//! only provider (PAT read from [`PAT_ENV`], attribution scope headers), the
//! request's model alias, the request's MCP servers, and `policies` that
//! confine the run. Tool limits are `tool.use` policies, never `permissions`,
//! because managed and generated `permissions` are overridable by an agent
//! definition while `policies` are not. [`PermissionMode::Plan`] denies `edit`,
//! `write` and `shell`. The run's gateway session comes from the PAT's session
//! binding, not from `x-session-id`: `OpenCode` 2 replaces that header with its
//! own session id, which the gateway records as the client session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::headers;
use systemprompt_models::bridge::opencode::{
    ApiKeyRef, Lockdown, McpOAuth, McpServer, ModelEntry, OpenCodeConfig, gateway_provider,
    lockdown_policies,
};

use super::PAT_ENV;
use crate::credentials::RunCredentials;
use crate::request::{McpServerSpec, PermissionMode, RunRequest};

pub const PROVIDER_ID: &str = "systemprompt";
pub const CLIENT_HEADER: &str = "headless-opencode";
pub const PLAN_DENIED_TOOLS: [&str; 3] = ["edit", "write", "shell"];

#[must_use]
pub fn build(
    request: &RunRequest,
    credentials: &RunCredentials,
    gateway_base_url: &str,
) -> OpenCodeConfig {
    let mut provider_headers = BTreeMap::from([
        (
            headers::SESSION_ID.to_owned(),
            credentials.gateway_session_id.as_str().to_owned(),
        ),
        (headers::CLIENT_KIND.to_owned(), CLIENT_HEADER.to_owned()),
    ]);
    provider_headers.extend(request.scope_headers());
    let models = request
        .model
        .iter()
        .map(|alias| {
            (
                alias.clone(),
                ModelEntry {
                    model: alias.clone(),
                    name: None,
                    limit: None,
                },
            )
        })
        .collect();
    let mut denied = request.disallowed_tools.clone();
    if request.permission_mode == PermissionMode::Plan {
        denied.extend(PLAN_DENIED_TOOLS.iter().map(|tool| (*tool).to_owned()));
    }
    let mcp_names: Vec<String> = request.mcp_servers.keys().cloned().collect();
    let mut config = OpenCodeConfig::default()
        .with_provider(
            PROVIDER_ID,
            gateway_provider(
                gateway_base_url,
                &ApiKeyRef::Env(PAT_ENV.to_owned()),
                provider_headers,
                models,
            ),
        )
        .with_policies(lockdown_policies(&Lockdown {
            provider: PROVIDER_ID,
            allowed_tools: &request.allowed_tools,
            denied_tools: &denied,
            allowed_mcp: &mcp_names,
            allowed_plugins: &[],
        }));
    if let Some(alias) = &request.model {
        config = config.with_model(PROVIDER_ID, alias);
    }
    for (name, spec) in &request.mcp_servers {
        config = config.with_mcp_server(name, mcp_server(spec));
    }
    config
}

fn mcp_server(spec: &McpServerSpec) -> McpServer {
    match spec {
        McpServerSpec::Stdio { command, args, env } => McpServer::Local {
            command: std::iter::once(command.clone())
                .chain(args.iter().cloned())
                .collect(),
            environment: env.clone(),
        },
        McpServerSpec::Http { url, headers } => McpServer::Remote {
            url: url.clone(),
            headers: headers.clone(),
            codemode: None,
            oauth: Some(McpOAuth::Disabled),
        },
    }
}
