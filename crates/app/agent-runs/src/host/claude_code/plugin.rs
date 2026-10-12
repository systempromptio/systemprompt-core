//! Host files for one run.
//!
//! The `--mcp-config` JSON is built from [`RunRequest::mcp_servers`] under
//! the run's `host/` directory; the governance plugin directory that
//! [`crate::governance::hooks_file`] already materialised, handed through
//! [`PrepareContext::hooks_file`] and loaded with `--plugin-dir`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{HeadlessError, HeadlessResult};
use crate::host::PrepareContext;
use crate::request::{McpServerSpec, RunRequest};

/// Files `prepare` wrote; each `None` means the flag is omitted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostFiles {
    pub mcp_config: Option<PathBuf>,
    pub governance_plugin: Option<PathBuf>,
}

/// The `--mcp-config` document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig<'a> {
    mcp_servers: BTreeMap<&'a str, McpEntry<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum McpEntry<'a> {
    Stdio {
        command: &'a str,
        args: &'a [String],
        env: &'a BTreeMap<String, String>,
    },
    Http {
        url: &'a str,
        headers: &'a BTreeMap<String, String>,
    },
}

#[must_use]
pub fn mcp_config_json(request: &RunRequest) -> McpConfig<'_> {
    let servers = request
        .mcp_servers
        .iter()
        .map(|(name, spec)| {
            let entry = match spec {
                McpServerSpec::Stdio { command, args, env } => {
                    McpEntry::Stdio { command, args, env }
                },
                McpServerSpec::Http { url, headers } => McpEntry::Http { url, headers },
            };
            (name.as_str(), entry)
        })
        .collect();
    McpConfig {
        mcp_servers: servers,
    }
}

pub fn write_host_files(ctx: &PrepareContext<'_>) -> HeadlessResult<HostFiles> {
    let host_dir = &ctx.layout.host;
    let mcp_config = if ctx.request.mcp_servers.is_empty() {
        None
    } else {
        let path = host_dir.join("mcp-config.json");
        write(
            &path,
            &serde_json::to_vec_pretty(&mcp_config_json(ctx.request))?,
        )?;
        Some(path)
    };
    Ok(HostFiles {
        mcp_config,
        governance_plugin: ctx.hooks_file.clone(),
    })
}

fn create_dir(path: &Path) -> HeadlessResult<()> {
    std::fs::create_dir_all(path).map_err(|source| layout_error(path, source))
}

fn write(path: &Path, bytes: &[u8]) -> HeadlessResult<()> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    std::fs::write(path, bytes).map_err(|source| layout_error(path, source))
}

fn layout_error(path: &Path, source: std::io::Error) -> HeadlessError {
    HeadlessError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
