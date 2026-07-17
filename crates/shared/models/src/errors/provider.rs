//! Boxed error and result aliases used by pluggable provider trait
//! abstractions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub type ProviderError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type ProviderResult<T> = Result<T, ProviderError>;
