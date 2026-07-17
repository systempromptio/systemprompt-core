//! Request-context extraction abstractions.
//!
//! Defines the [`ContextExtractor`] trait that the context middleware uses to
//! derive a `RequestContext` from request headers or body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod traits;

pub use traits::ContextExtractor;
