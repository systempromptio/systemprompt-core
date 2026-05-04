//! Application context, module registry, and request-context propagation.
//!
//! The traits in this module are the runtime entry points other crates use
//! to discover configuration, the database handle, and the registered
//! providers (analytics, fingerprint, user). [`ContextPropagation`] models
//! how request-scoped state moves across HTTP boundaries.

use async_trait::async_trait;
use std::sync::Arc;

use crate::analytics::{AnalyticsProvider, FingerprintProvider};
use crate::auth::UserProvider;

/// Top-level handle to the running application.
///
/// Every crate that needs configuration, database access, or shared
/// providers obtains them via this trait, allowing the binary entry point
/// to swap concrete implementations without leaking those details to
/// downstream consumers.
pub trait AppContext: Send + Sync {
    /// Return the active configuration provider.
    fn config(&self) -> Arc<dyn ConfigProvider>;
    /// Return the shared database handle.
    fn database_handle(&self) -> Arc<dyn DatabaseHandle>;
    /// Return the analytics provider, if one is registered.
    fn analytics_provider(&self) -> Option<Arc<dyn AnalyticsProvider>>;
    /// Return the fingerprint provider, if one is registered.
    fn fingerprint_provider(&self) -> Option<Arc<dyn FingerprintProvider>>;
    /// Return the user provider, if one is registered.
    fn user_provider(&self) -> Option<Arc<dyn UserProvider>>;
}

/// Inject the request context represented by this value into outgoing HTTP
/// headers.
pub trait InjectContextHeaders {
    /// Mutate `headers` to embed this context for downstream services.
    fn inject_headers(&self, headers: &mut http::HeaderMap);
}

/// Result alias for [`ContextPropagation::from_headers`].
pub type ContextPropagationResult<T> = Result<T, ContextPropagationError>;

/// Errors returned when extracting a request context from HTTP headers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextPropagationError {
    /// A required header was absent from the request.
    #[error("missing header: {0}")]
    MissingHeader(String),

    /// A header value was present but could not be decoded.
    #[error("invalid header {name}: {message}")]
    InvalidHeader {
        /// Name of the offending header.
        name: String,
        /// Human-readable description of why the value was rejected.
        message: String,
    },

    /// The combined header set failed a higher-level validation rule.
    #[error("invalid context: {0}")]
    Invalid(String),
}

/// Two-way conversion between a typed request context and HTTP headers.
///
/// Implementors describe how their context is serialised onto an outbound
/// request and parsed back on the receiving side. Returning typed
/// [`ContextPropagationError`] values lets transport layers map specific
/// failures (missing header, malformed value) to protocol-appropriate
/// responses.
pub trait ContextPropagation {
    /// Reconstruct the context from the supplied request `headers`.
    ///
    /// # Errors
    /// Returns a [`ContextPropagationError`] when a required header is
    /// missing or a value cannot be parsed.
    fn from_headers(headers: &http::HeaderMap) -> ContextPropagationResult<Self>
    where
        Self: Sized;

    /// Serialise the context into a fresh header map.
    fn to_headers(&self) -> http::HeaderMap;
}

/// Read-only access to runtime configuration values.
///
/// The trait keeps the concrete configuration type opaque so callers can
/// downcast through [`as_any`](Self::as_any) when they own the
/// implementation, but rely on the documented accessors otherwise.
pub trait ConfigProvider: Send + Sync {
    /// Look up an arbitrary configuration key.
    fn get(&self, key: &str) -> Option<String>;
    /// Return the primary database connection URL.
    fn database_url(&self) -> &str;
    /// Optional dedicated write-side database URL.
    fn database_write_url(&self) -> Option<&str> {
        None
    }
    /// Filesystem root for the runtime ("system path").
    fn system_path(&self) -> &str;
    /// Bound port for the HTTP API server.
    fn api_port(&self) -> u16;
    /// Type-erased accessor for downcasting to the concrete provider.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Discovery interface for runtime modules registered with the application.
pub trait ModuleRegistry: Send + Sync {
    /// Return the module registered under `name`, if any.
    fn get_module(&self, name: &str) -> Option<Arc<dyn Module>>;
    /// List the names of every registered module.
    fn list_modules(&self) -> Vec<String>;
}

/// Health-checked database handle used as a runtime capability.
pub trait DatabaseHandle: Send + Sync {
    /// Report whether the underlying pool currently believes it is connected.
    fn is_connected(&self) -> bool;
    /// Type-erased accessor for downcasting to the concrete pool type.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// A pluggable runtime module.
///
/// `#[async_trait]` is required because `Module` is consumed as
/// `Arc<dyn Module>` by [`ModuleRegistry`].
#[async_trait]
pub trait Module: Send + Sync {
    /// Stable identifier for the module.
    fn name(&self) -> &str;
    /// Semantic version string.
    fn version(&self) -> &str;
    /// Human-friendly display name.
    fn display_name(&self) -> &str;
    /// Run any startup work required before the module is exposed.
    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>>;
}

/// Module variant that contributes an `axum` router to the API server.
///
/// Only available when the `web` feature is enabled.
#[cfg(feature = "web")]
#[async_trait]
pub trait ApiModule: Module {
    /// Build the router this module exposes, given the application context.
    fn router(&self, ctx: Arc<dyn AppContext>) -> axum::Router;
}
