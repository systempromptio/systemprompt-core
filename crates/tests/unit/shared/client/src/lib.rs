//! Unit tests for systemprompt-client crate
//!
//! Tests cover:
//! - ClientError creation, variants, and is_retryable logic
//! - SystempromptClient construction, token management, and API methods
//! - HTTP request handling (GET, POST, PUT, DELETE)
//! - Error response parsing and network error handling

#[cfg(test)]
mod client;
#[cfg(test)]
mod error;
#[cfg(test)]
mod http;
