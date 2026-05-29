//! Request-context middleware: establishing the per-request execution identity.
//!
//! Combines the [`ContextExtractor`] abstraction with the per-route middleware
//! flavours ([`PublicContextMiddleware`], [`UserOnlyContextMiddleware`],
//! [`A2AContextMiddleware`], [`McpContextMiddleware`]) and the context-id
//! sources ([`HeaderSource`], [`PayloadSource`]) that feed them.

pub mod extractors;
pub mod middleware;
pub mod sources;

pub use extractors::ContextExtractor;
pub use middleware::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
pub use sources::{HeaderSource, PayloadSource};
