//! What an engine is built from, filled by the shell at startup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use std::fmt;
use std::sync::Arc;

use crate::config::{Layout, Paths};
use crate::llm::Transport;
use crate::mcp::ClientInfo;
use crate::store::Store;

use crate::run::extension::ExtensionSet;
use crate::run::governor::Governor;

/// Everything an [`Engine`](crate::run::Engine) is built from.
///
/// Nothing is read from the environment by the engine itself. `client_info` is
/// how the app introduces itself to MCP servers, `search_path` is the program
/// search path for formatters and language servers, and `path_ext` lists the
/// executable extensions tried on Windows when looking programs up.
/// `extensions` is every linked engine extension, resolved; what each one
/// contributes reaches a session only while the Governor's enablement admits
/// it.
pub struct EngineOptions {
    pub paths: Paths,
    pub layout: Layout,
    pub store: Store,
    pub governor: Arc<dyn Governor>,
    pub transport: Arc<dyn Transport>,
    pub client_info: ClientInfo,
    pub search_path: Option<OsString>,
    pub path_ext: Option<OsString>,
    pub extensions: ExtensionSet,
    pub lsp_client_name: Option<String>,
}

impl fmt::Debug for EngineOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineOptions")
            .field("paths", &self.paths)
            .field("layout", &self.layout)
            .field("store", &self.store)
            .field("client_info", &self.client_info)
            .field("extensions", &self.extensions)
            .field("lsp_client_name", &self.lsp_client_name)
            .finish_non_exhaustive()
    }
}
