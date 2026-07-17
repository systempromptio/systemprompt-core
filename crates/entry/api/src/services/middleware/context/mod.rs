//! Request-context middleware: establishing the per-request execution identity.
//!
//! Combines the [`ContextExtractor`] abstraction with the per-route middleware
//! flavours ([`PublicContextMiddleware`], [`UserOnlyContextMiddleware`],
//! [`A2AContextMiddleware`], [`McpContextMiddleware`]) and the context-id
//! sources ([`HeaderSource`], [`PayloadSource`]) that feed them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extractors;
pub mod middleware;
pub mod sources;

pub use extractors::ContextExtractor;
pub use middleware::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
pub use sources::{HeaderSource, PayloadSource};
