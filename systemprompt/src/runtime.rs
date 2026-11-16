//! The document store runtime for a binary that embeds systemprompt.
//!
//! A custom binary composes its members and runs the CLI through
//! [`Composition::run`](crate::Composition::run). With `api` on,
//! [`documents`] re-exports the shared document store runtime:
//! `DocumentsRuntime` and the MCP tools an extension's server binary
//! composes, `documents::mcp::DocumentsMcpTools`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub use systemprompt_runtime::documents;
