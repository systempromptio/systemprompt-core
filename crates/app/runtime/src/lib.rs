//! `systemprompt-runtime` — application runtime services.
//!
//! This crate hosts [`AppContext`], the lifecycle [`AppContextBuilder`],
//! per-module installation helpers, startup validation, and the typed
//! [`RuntimeError`] / [`RuntimeResult`] error boundary used by all of
//! the above.
//!
//! Public APIs return [`RuntimeResult<T>`]. [`RuntimeError`] composes
//! upstream typed errors (`ConfigError`, `RepositoryError`,
//! `FilesError`, `UserError`, `LoaderError`, `AnalyticsError`,
//! `ProfileBootstrapError`, `PathError`) via `#[from]`; every variant
//! built from another error keeps it as its source.
//!
//! # Feature flags
//!
//! | Feature       | Effect                                                          |
//! |---------------|------------------------------------------------------------------|
//! | (default)     | Core context, builder, validation                               |
//! | `geolocation` | Enables `MaxMind` `GeoIP2` loading via `maxminddb` and pulls in `systemprompt-analytics/geolocation` |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builder;
mod context;
mod context_traits;
mod database_context;
mod error;
pub mod managed;
pub mod services_reconcile;
mod startup_validation;
pub mod trace;
mod validation;

pub use builder::{AppContextBuilder, discover_models, owner_reassignments};
pub use context::{AppContext, ConfigPlane, DataPlane, Plugins, ShutdownRequest, Subsystems};
pub use database_context::DatabaseContext;
pub use error::{RuntimeError, RuntimeResult};
pub use startup_validation::{
    ExtensionConfigOutcome, FilesConfigValidator, StartupValidator, collect_manifest_errors,
    display_validation_report, display_validation_warnings, merge_mcp_errors,
    validate_extension_configs,
};
pub use systemprompt_database::MigrationConfig;
pub use trace::{
    AiRequestClientEvidence, AiRequestDetail, AiRequestFilter, AiRequestInfo, AiRequestListItem,
    AiRequestStats, AiRequestSummary, AiTraceService, AuditLookupResult, AuditPage,
    AuditToolCallRow, ConversationMessage, ExecutionStep, ExecutionStepSummary, LevelCount,
    LinkedMcpCall, LogSearchFilter, LogSearchItem, LogTimeRange, McpExecutionSummary,
    McpToolExecution, ModelStatsRow, ModuleCount, ProviderStatsRow, RequestCursor,
    RequestCursorError, TaskArtifact, TaskInfo, ToolExecutionFilter, ToolExecutionItem,
    ToolLogEntry, TraceError, TraceEvent, TraceListFilter, TraceListItem, TraceQueryService,
};
pub use validation::{validate_database_url, validate_system};

pub use systemprompt_models::modules::ServiceCategory;
