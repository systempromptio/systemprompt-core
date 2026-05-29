//! Request-context extraction abstractions.
//!
//! Defines the [`ContextExtractor`] trait that the context middleware uses to
//! derive a `RequestContext` from request headers or body.

pub mod traits;

pub use traits::ContextExtractor;
