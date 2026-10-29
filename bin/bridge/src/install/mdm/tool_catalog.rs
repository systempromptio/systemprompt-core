//! The tool names each managed MCP server last reported, kept so Claude
//! Desktop's per-tool `toolPolicy` can be written for a server whose manifest
//! entry says "every tool".
//!
//! Claude Desktop has no server-wide switch: `managedMcpServers[].toolPolicy`
//! names tools one by one. The bridge learns the names through its MCP auth
//! probe (`initialize` → `tools/list`) and keeps only what a probe of each
//! server confirmed: a server whose probe is refused loses its names, so a
//! wildcard over it is withheld rather than expanded over a list that may
//! miss new tools, while an inconclusive probe (timeout, proxy or local
//! fault) keeps the names last confirmed. Servers whose gateway reports the
//! user's account as not connected are recorded separately, so the policy
//! writer can say so instead of reporting a catalog gap. An absent file is an
//! empty catalog; an unreadable or corrupt one is an error, never an empty
//! catalog. A catalog that could not be refreshed is invalidated (removed) so
//! no policy is projected over it. Servers that leave the manifest are dropped
//! so a retired server's names never leak into a later policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::proxy::mcp_probe::{McpAuthState, McpServerAuth};

const FILE: &str = "mcp-tools.json";
const NEEDS_CONNECTION_FILE: &str = "mcp-needs-connection.json";

#[derive(Debug, thiserror::Error)]
enum CatalogFileError {
    #[error("{}: {source}", .path.display())]
    Decode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type ToolCatalog = BTreeMap<String, Vec<String>>;

fn path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|dir| dir.join(FILE))
}

pub fn read() -> std::io::Result<ToolCatalog> {
    let Some(path) = path() else {
        return Ok(ToolCatalog::new());
    };
    match std::fs::read_to_string(&path) {
        Ok(body) => serde_json::from_str(&body).map_err(|source| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                CatalogFileError::Decode { path, source },
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ToolCatalog::new()),
        Err(source) => Err(std::io::Error::new(
            source.kind(),
            CatalogFileError::Read { path, source },
        )),
    }
}

pub fn record(results: &[McpServerAuth]) -> std::io::Result<ToolCatalog> {
    let mut catalog = read()?;
    for result in results {
        if result.id.is_empty() {
            continue;
        }
        if is_inconclusive(result.state) {
            continue;
        }
        if result.state != McpAuthState::Authenticated {
            catalog.remove(&result.id);
            continue;
        }
        let mut names: Vec<String> = result.tools.iter().map(|t| t.name.clone()).collect();
        names.sort();
        names.dedup();
        catalog.insert(result.id.clone(), names);
    }
    if let Some(path) = path() {
        let body = serde_json::to_string_pretty(&catalog).map_err(std::io::Error::other)?;
        crate::fsutil::atomic_write_0644(&path, format!("{body}\n").as_bytes())?;
    }
    let needing: std::collections::BTreeSet<String> = results
        .iter()
        .filter(|r| r.state.needs_account_connection())
        .map(|r| r.id.clone())
        .collect();
    if let Some(dir) = crate::config::paths::bridge_metadata_dir() {
        let body = serde_json::to_vec(&needing).map_err(std::io::Error::other)?;
        crate::fsutil::atomic_write_0644(&dir.join(NEEDS_CONNECTION_FILE), &body)?;
    }
    Ok(catalog)
}

const fn is_inconclusive(state: McpAuthState) -> bool {
    matches!(
        state,
        McpAuthState::Unknown
            | McpAuthState::ProxyUnreachable
            | McpAuthState::ProbeTimeout
            | McpAuthState::LocalError
    )
}

#[must_use]
pub fn needs_connection() -> std::collections::BTreeSet<String> {
    crate::config::paths::bridge_metadata_dir()
        .and_then(|dir| std::fs::read(dir.join(NEEDS_CONNECTION_FILE)).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn retain(slugs: &[String]) -> std::io::Result<()> {
    let mut catalog = read()?;
    let before = catalog.len();
    catalog.retain(|slug, _| slugs.contains(slug));
    if catalog.len() == before {
        return Ok(());
    }
    if let Some(path) = path() {
        let body = serde_json::to_string_pretty(&catalog).map_err(std::io::Error::other)?;
        crate::fsutil::atomic_write_0644(&path, format!("{body}\n").as_bytes())?;
    }
    Ok(())
}

pub fn invalidate() -> std::io::Result<()> {
    path().map_or(Ok(()), |path| crate::fsutil::remove_verified(&path))
}
