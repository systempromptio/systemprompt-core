//! The issue report tools a hosted MCP server serves, built from the host's
//! kernel state: the loaded files configuration and the bound document store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_documents::bound_handles;
use systemprompt_documents::links::{LinkIssueError, LinkIssuer};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsUnbound;
use systemprompt_identifiers::McpServerId;

use super::IssuesMcpTools;

/// Why the issue report tools could not be built for a hosted server.
#[derive(Debug, thiserror::Error)]
pub enum IssuesMcpError {
    #[error(
        "MCP server {server_name} serves issue report tools but issue reports are disabled: set \
         files.issues.enabled: true in services/config/files.yaml"
    )]
    Disabled { server_name: McpServerId },
    #[error("MCP server {server_name}: the host holds no files configuration")]
    MissingConfig { server_name: McpServerId },
    #[error("MCP server {server_name}: the API link client could not be built")]
    Links {
        server_name: McpServerId,
        #[source]
        source: LinkIssueError,
    },
    #[error("MCP server {server_name}: the document store is not bound on this host")]
    Unbound {
        server_name: McpServerId,
        #[source]
        source: DocumentsUnbound,
    },
}

impl IssuesMcpTools {
    pub fn hosted(
        state: &Dependencies,
        links: impl FnOnce() -> Result<Arc<dyn LinkIssuer>, LinkIssueError>,
        server_name: McpServerId,
    ) -> Result<Self, IssuesMcpError> {
        let Some(config) = state.find::<Arc<FilesConfig>>() else {
            return Err(IssuesMcpError::MissingConfig { server_name });
        };
        if !config.issues_config().is_enabled() {
            return Err(IssuesMcpError::Disabled { server_name });
        }
        let handles = bound_handles(state).map_err(|source| IssuesMcpError::Unbound {
            server_name: server_name.clone(),
            source,
        })?;
        let uploads = config
            .documents_config()
            .is_enabled()
            .then(|| Arc::clone(&handles.uploads));
        let links = links().map_err(|source| IssuesMcpError::Links {
            server_name: server_name.clone(),
            source,
        })?;
        Ok(Self::new(
            Arc::clone(&handles.issues),
            uploads,
            links,
            server_name,
        ))
    }
}
