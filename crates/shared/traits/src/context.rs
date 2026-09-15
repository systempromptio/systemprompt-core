//! Application context, configuration and database handle contracts, and
//! request-context propagation across HTTP boundaries.
//!
//! [`AppContext`] is the runtime entry point the HTTP layer uses to reach
//! the registered providers (analytics, fingerprint, user) without naming
//! the concrete runtime type; [`ContextPropagation`] models how
//! request-scoped state moves across HTTP boundaries; [`ConfigProvider`] and
//! [`DatabaseHandle`] are the two capabilities the extension framework hands
//! to downstream code without exposing a concrete pool or profile type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::analytics::{AnalyticsProvider, FingerprintProvider};
use crate::auth::UserProvider;

pub trait AppContext: Send + Sync {
    fn config(&self) -> Arc<dyn ConfigProvider>;
    fn database_handle(&self) -> Arc<dyn DatabaseHandle>;
    fn session_provider(&self) -> Option<Arc<dyn crate::SessionProvider>>;
    fn analytics_provider(&self) -> Option<Arc<dyn AnalyticsProvider>>;
    fn fingerprint_provider(&self) -> Option<Arc<dyn FingerprintProvider>>;
    fn user_provider(&self) -> Option<Arc<dyn UserProvider>>;
}

pub trait InjectContextHeaders {
    fn inject_headers(&self, headers: &mut http::HeaderMap);
}

pub type ContextPropagationResult<T> = Result<T, ContextPropagationError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ContextPropagationError {
    #[error("missing header: {0}")]
    MissingHeader(String),

    #[error("invalid header {name}: {message}")]
    InvalidHeader { name: String, message: String },

    #[error("invalid context: {0}")]
    Invalid(String),
}

pub trait ContextPropagation {
    fn from_headers(headers: &http::HeaderMap) -> ContextPropagationResult<Self>
    where
        Self: Sized;

    fn to_headers(&self) -> http::HeaderMap;
}

pub trait ConfigProvider: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn database_url(&self) -> &str;
    fn database_write_url(&self) -> Option<&str> {
        None
    }
    fn system_path(&self) -> &str;
    fn api_port(&self) -> u16;
    fn as_any(&self) -> &dyn std::any::Any;
}

pub trait DatabaseHandle: Send + Sync {
    fn is_connected(&self) -> bool;
    fn as_any(&self) -> &dyn std::any::Any;
}
