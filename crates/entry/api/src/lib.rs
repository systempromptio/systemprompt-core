#![allow(
    clippy::unused_async,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::missing_const_for_fn,
    clippy::clone_on_ref_ptr,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    clippy::option_if_let_else,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
    clippy::redundant_clone,
    clippy::expect_used,
    clippy::type_complexity,
    clippy::unnecessary_wraps
)]

pub mod models;
pub mod routes;
pub mod services;

pub use models::ServerConfig;
pub use services::health::{HealthChecker, HealthSummary, ModuleHealth, ProcessMonitor};
pub use services::middleware::{ContextExtractor, ContextMiddleware, HeaderContextExtractor};
pub use services::server::ApiServer;
