//! Embedding helper for the `systemprompt` CLI.
//!
//! [`RuntimeBuilder`] hands control to the standard CLI entry point from a
//! custom binary. Extensions register at compile time through
//! `systemprompt_extension::register_extension!`; a binary that links an
//! extension crate carries it, and discovery finds it through `inventory`.
//!
//! With `api` on, [`documents`] re-exports the shared document store runtime:
//! `DocumentsRuntime` and the MCP tools an extension's server binary
//! composes, `documents::mcp::DocumentsMcpTools`.
//!
//! ```no_run
//! # #[cfg(all(feature = "runtime", feature = "core"))]
//! # async fn demo() -> Result<(), systemprompt::RuntimeError> {
//! use systemprompt::RuntimeBuilder;
//!
//! RuntimeBuilder::new().run().await
//! # }
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub use systemprompt_runtime::documents;

/// Typed error returned by [`RuntimeBuilder::run`].
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("CLI exited with error: {0}")]
    Cli(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Entry point for a custom binary that embeds the CLI.
#[derive(Debug, Default, Clone, Copy)]
pub struct RuntimeBuilder;

impl RuntimeBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn run(self) -> Result<(), RuntimeError> {
        Box::pin(systemprompt_cli::run())
            .await
            .map_err(|err| RuntimeError::Cli(err.into()))
    }
}
