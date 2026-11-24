//! `systemprompt-runtime` — application runtime services.
//!
//! This crate hosts [`AppContext`], the lifecycle [`AppContextBuilder`],
//! per-module installation helpers, startup validation, and the typed
//! [`RuntimeError`] / [`RuntimeResult`] error boundary used by all of
//! the above.
//!
//! Public APIs return [`RuntimeResult<T>`]. [`RuntimeError`] composes
//! upstream typed errors (`ConfigError`, `RepositoryError`,
//! `FilesError`, `LoaderError`,
//! `ProfileBootstrapError`, `PathError`) via `#[from]`; every variant
//! built from another error keeps it as its source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin_bootstrap;
mod builder;
mod context;
mod context_traits;
mod database_context;
mod error;
pub mod host_kernel;
mod kernel;
mod resolved_impl;
pub mod schema_currency;
pub mod services_reconcile;
mod startup_validation;
pub mod storage;
pub mod trace;
mod validation;

pub use admin_bootstrap::{ADMIN_BOOTSTRAP_OWNER, AdminBootstrapUnavailable, admin_bootstrap};
pub use builder::kernel_binders::{
    ApiKernelBinders, ApiLateBinders, KernelBinders, PendingBinders,
};
pub use builder::kernel_state::{
    ApiKernelSeed, KernelCoordination, KernelSeed, base_kernel_state, kernel_state,
};
pub use builder::{AppContextBuilder, discover_vertex_models};
pub use context::{
    AppContext, ConfigPlane, ContextPlanes, DataPlane, Plugins, ShutdownRequest, Subsystems,
};
pub use database_context::DatabaseContext;
pub use error::{RuntimeError, RuntimeResult};
pub use host_kernel::{AdmitFn, HostBootError, HostKernel, HostParts, PROFILE_ENV};
pub use kernel::{Kernel, KernelError};
pub use schema_currency::{SchemaState, assert_schema_current};
pub use startup_validation::{
    StartupValidator, collect_manifest_errors, display_validation_report,
    display_validation_warnings, merge_mcp_errors,
};
pub use systemprompt_database::MigrationConfig;

pub use systemprompt_provider_contracts::{ConfigPlaneSet, EffectiveAuthority};
pub use trace::{
    AiRequestClientEvidence, AiRequestDetail, AiRequestFilter, AiRequestInfo, AiRequestListItem,
    AiRequestStats, AiRequestSummary, AiTraceService, AuditLookupResult, AuditPage,
    AuditToolCallRow, ConversationMessage, ExecutionStep, ExecutionStepSummary, LevelCount,
    LinkedMcpCall, LogSearchFilter, LogSearchItem, LogTimeRange, McpExecutionSummary,
    McpToolExecution, ModelStatsRow, ModuleCount, ProviderStatsRow, RequestCursor,
    RequestCursorError, TaskArtifact, TaskInfo, ToolExecutionFilter, ToolExecutionItem,
    ToolLogEntry, TraceError, TraceEvent, TraceListFilter, TraceListItem, TraceQueryService,
    TraceRepository,
};
pub use validation::{validate_database_url, validate_system};

pub use systemprompt_models::modules::ServiceCategory;
