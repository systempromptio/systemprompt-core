//! The document tools a hosted MCP server serves, built from the host's
//! kernel state: the loaded files configuration, the bound document store
//! and the user lookup the `documents` member provides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsUnbound;
use systemprompt_identifiers::McpServerId;
use systemprompt_provider_contracts::Dependencies;

use super::DocumentsMcpTools;
use crate::bound_handles;
use crate::links::{LinkIssueError, LinkIssuer};

/// Why the document tools could not be built for a hosted server.
#[derive(Debug, thiserror::Error)]
pub enum DocumentsMcpError {
    #[error(
        "MCP server {server_name} serves document tools but the document store is disabled: set \
         files.documents.enabled: true in services/config/files.yaml"
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

impl DocumentsMcpTools {
    pub fn hosted(
        state: &Dependencies,
        links: impl FnOnce() -> Result<Arc<dyn LinkIssuer>, LinkIssueError>,
        server_name: McpServerId,
    ) -> Result<Self, DocumentsMcpError> {
        let Some(config) = state.find::<Arc<FilesConfig>>() else {
            return Err(DocumentsMcpError::MissingConfig { server_name });
        };
        if !config.documents_config().is_enabled() {
            return Err(DocumentsMcpError::Disabled { server_name });
        }
        let links = links().map_err(|source| DocumentsMcpError::Links {
            server_name: server_name.clone(),
            source,
        })?;
        let handles = bound_handles(state).map_err(|source| DocumentsMcpError::Unbound {
            server_name: server_name.clone(),
            source,
        })?;
        Self::for_host(&handles, state, links, server_name.clone())
            .ok_or(DocumentsMcpError::Disabled { server_name })
    }
}
