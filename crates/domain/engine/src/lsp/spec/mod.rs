//! Which servers exist and which of them serve a given file.
//!
//! Configured servers come first and replace a built-in of the same id. A
//! built-in is offered only when its binary is found on the search path the
//! shell injects. Built-ins that serve the same language as alternatives (for
//! example two Python servers) form a group in which the first one found wins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builtins;
mod resolve;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use builtins::builtin_servers;
pub(crate) use resolve::{MissingCommand, Resolved, resolve_servers, select};

/// Identifies a server definition, such as `rust-analyzer`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServerId(String);

impl ServerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ServerId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

impl From<String> for ServerId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

/// How to launch one language server and which files it serves.
///
/// `extensions` are matched case-insensitively, with or without a leading dot;
/// an entry may also be a whole file name. `root_markers` are tried in order:
/// for each marker the directories from the file up to the worktree are
/// searched, and the first directory holding it becomes the server's root. A
/// marker starting with `*` matches any entry with that suffix (`*.csproj`).
/// With no marker found the worktree is the root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerSpec {
    pub id: ServerId,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub root_markers: Vec<String>,
    // JSON: server-defined `initializationOptions`, also answered to `workspace/configuration`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<Value>,
}

impl ServerSpec {
    pub fn handles(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .map(|ext| ext.to_string_lossy().into_owned());
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        self.extensions.iter().any(|wanted| {
            let bare = wanted.strip_prefix('.').unwrap_or(wanted);
            ext.as_deref()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(bare))
                || name.as_deref().is_some_and(|name| name == wanted.as_str())
        })
    }
}

/// Time limits for talking to servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspTimeouts {
    pub initialize: Duration,
    pub request: Duration,
    pub diagnostics_debounce: Duration,
    pub diagnostics_cap: Duration,
    pub shutdown: Duration,
}

impl Default for LspTimeouts {
    fn default() -> Self {
        Self {
            initialize: Duration::from_secs(45),
            request: Duration::from_secs(15),
            diagnostics_debounce: Duration::from_millis(250),
            diagnostics_cap: Duration::from_secs(5),
            shutdown: Duration::from_secs(2),
        }
    }
}

/// Everything an [`crate::LspManager`] is built from.
///
/// `search_path` is the `PATH` value the shell read at startup; without it only
/// absolute or worktree-relative commands resolve and no built-in is offered.
/// `path_ext` is Windows' `PATHEXT`, defaulting to the system list.
/// `client_name` and `client_version` are the `clientInfo` the client presents
/// to every server.
#[derive(Debug, Clone)]
pub struct LspConfig {
    pub builtins: bool,
    pub servers: Vec<ServerSpec>,
    pub disabled: Vec<ServerId>,
    pub search_path: Option<OsString>,
    pub path_ext: Option<OsString>,
    pub timeouts: LspTimeouts,
    pub client_name: String,
    pub client_version: String,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            builtins: true,
            servers: Vec::new(),
            disabled: Vec::new(),
            search_path: None,
            path_ext: None,
            timeouts: LspTimeouts::default(),
            client_name: env!("CARGO_PKG_NAME").to_owned(),
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}
