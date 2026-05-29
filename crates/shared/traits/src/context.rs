//! Application context, module registry, and request-context propagation.
//!
//! The async traits here are dispatched as trait objects (`dyn _`), so they
//! use `#[async_trait]`; native `async fn` in traits is not yet
//! `dyn`-compatible.
//!
//! The traits in this module are the runtime entry points other crates use
//! to discover configuration, the database handle, and the registered
//! providers (analytics, fingerprint, user). [`ContextPropagation`] models
//! how request-scoped state moves across HTTP boundaries.

use async_trait::async_trait;
use std::sync::Arc;

use crate::analytics::{AnalyticsProvider, FingerprintProvider};
use crate::auth::UserProvider;

pub trait AppContext: Send + Sync {
    fn config(&self) -> Arc<dyn ConfigProvider>;
    fn database_handle(&self) -> Arc<dyn DatabaseHandle>;
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

pub trait ModuleRegistry: Send + Sync {
    fn get_module(&self, name: &str) -> Option<Arc<dyn Module>>;
    fn list_modules(&self) -> Vec<String>;
}

pub trait DatabaseHandle: Send + Sync {
    fn is_connected(&self) -> bool;
    fn as_any(&self) -> &dyn std::any::Any;
}

#[async_trait]
pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn display_name(&self) -> &str;
    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>>;
}

#[cfg(feature = "web")]
#[async_trait]
pub trait ApiModule: Module {
    fn router(&self, ctx: Arc<dyn AppContext>) -> axum::Router;
}
