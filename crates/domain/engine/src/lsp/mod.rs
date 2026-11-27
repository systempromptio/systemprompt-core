//! Language server client for the agent engine.
//!
//! An [`LspManager`] belongs to one worktree. It starts the language servers a
//! file needs on first use (chosen by extension, rooted at the nearest root
//! marker), keeps the server's view of touched files in sync with the disk,
//! collects published diagnostics and answers symbol queries. Servers come from
//! configuration plus a built-in table of well-known servers that is used only
//! when their binary is already on the search path; nothing is downloaded.
//!
//! Positions are zero-based lines and UTF-16 code-unit columns, as the protocol
//! defines them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod error;
mod format;
mod framing;
mod language;
mod manager;
/// Readers of protocol JSON into the client's own types.
pub mod parse;
mod paths;
mod spec;
mod types;
mod uri;

pub use error::LspError;
pub use format::{DiagnosticFormat, format_diagnostics};
pub use framing::{FrameDecoder, FrameError, encode_frame};
pub use language::language_id;
pub use manager::LspManager;
pub use spec::{LspConfig, LspTimeouts, ServerId, ServerSpec, builtin_servers};
pub use types::{
    Diagnostic, DiagnosticScope, Hover, Location, Position, Range, ServerState, ServerStatus,
    Severity, Symbol, SymbolKind,
};
pub use uri::{PathStyle, decode_uri, encode_path, path_to_uri, uri_to_path};
