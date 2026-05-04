//! Boxed error and result aliases used by pluggable provider trait
//! abstractions.

pub type ProviderError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type ProviderResult<T> = Result<T, ProviderError>;
