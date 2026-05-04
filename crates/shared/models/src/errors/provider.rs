//! Boxed error and result aliases used by pluggable provider trait
//! abstractions.

/// Boxed error returned by pluggable provider trait abstractions
/// (`AiProvider`, `McpRegistry`, `McpToolProvider`, `McpDeploymentProvider`).
///
/// The concrete failure type depends on the backend implementation
/// (HTTP client error, MCP protocol error, codec error, …) and is held
/// behind a thread-safe trait object so the trait surface stays
/// implementation-agnostic.
pub type ProviderError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Convenience `Result` alias for provider trait abstractions that
/// return [`ProviderError`].
pub type ProviderResult<T> = Result<T, ProviderError>;
