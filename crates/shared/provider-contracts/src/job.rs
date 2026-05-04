//! [`Job`] contract for scheduled / on-startup background jobs registered
//! via the `inventory` crate.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::ProviderResult;

/// Outcome of a single [`Job::execute`] call.
#[derive(Debug, Clone)]
pub struct JobResult {
    /// Whether the job completed without surfacing a failure.
    pub success: bool,
    /// Optional human-readable status / error detail.
    pub message: Option<String>,
    /// Items the job processed in this run, when meaningful.
    pub items_processed: Option<u64>,
    /// Items the job failed to process in this run, when meaningful.
    pub items_failed: Option<u64>,
    /// Wall-clock duration of the run, in milliseconds.
    pub duration_ms: u64,
}

impl JobResult {
    /// Build a successful [`JobResult`] with no extra detail attached.
    #[must_use]
    pub const fn success() -> Self {
        Self {
            success: true,
            message: None,
            items_processed: None,
            items_failed: None,
            duration_ms: 0,
        }
    }

    /// Attach a human-readable message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Attach processed / failed item counts.
    #[must_use]
    pub const fn with_stats(mut self, processed: u64, failed: u64) -> Self {
        self.items_processed = Some(processed);
        self.items_failed = Some(failed);
        self
    }

    /// Attach the run's wall-clock duration.
    #[must_use]
    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Build a failure [`JobResult`] with the given message.
    #[must_use]
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            items_processed: None,
            items_failed: None,
            duration_ms: 0,
        }
    }
}

/// Per-run context handed to [`Job::execute`].
///
/// Wraps three type-erased host objects (database pool, app context, app
/// paths) so the contract crate does not need to depend on their concrete
/// types; jobs downcast at the use site.
pub struct JobContext {
    db_pool: Arc<dyn std::any::Any + Send + Sync>,
    app_context: Arc<dyn std::any::Any + Send + Sync>,
    app_paths: Arc<dyn std::any::Any + Send + Sync>,
    parameters: HashMap<String, String>,
}

impl std::fmt::Debug for JobContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobContext")
            .field("db_pool", &"<type-erased>")
            .field("app_context", &"<type-erased>")
            .field("app_paths", &"<type-erased>")
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl JobContext {
    /// Build a [`JobContext`] from type-erased host handles.
    #[must_use]
    pub fn new(
        db_pool: Arc<dyn std::any::Any + Send + Sync>,
        app_context: Arc<dyn std::any::Any + Send + Sync>,
        app_paths: Arc<dyn std::any::Any + Send + Sync>,
    ) -> Self {
        Self {
            db_pool,
            app_context,
            app_paths,
            parameters: HashMap::new(),
        }
    }

    /// Attach per-run string parameters parsed from the schedule entry.
    #[must_use]
    pub fn with_parameters(mut self, parameters: HashMap<String, String>) -> Self {
        self.parameters = parameters;
        self
    }

    /// Type-erased downcast of the host's database pool.
    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.as_ref().downcast_ref::<T>()
    }

    /// Type-erased downcast of the host's `AppContext`.
    #[must_use]
    pub fn app_context<T: 'static>(&self) -> Option<&T> {
        self.app_context.as_ref().downcast_ref::<T>()
    }

    /// Type-erased downcast of the host's `AppPaths`.
    #[must_use]
    pub fn app_paths<T: 'static>(&self) -> Option<&T> {
        self.app_paths.as_ref().downcast_ref::<T>()
    }

    /// Cloned `Arc` to the type-erased database pool.
    #[must_use]
    pub fn db_pool_arc(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::clone(&self.db_pool)
    }

    /// Cloned `Arc` to the type-erased `AppContext`.
    #[must_use]
    pub fn app_context_arc(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::clone(&self.app_context)
    }

    /// Cloned `Arc` to the type-erased `AppPaths`.
    #[must_use]
    pub fn app_paths_arc(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::clone(&self.app_paths)
    }

    /// Borrow the per-run parameters map.
    #[must_use]
    pub const fn parameters(&self) -> &HashMap<String, String> {
        &self.parameters
    }

    /// Look up a single per-run parameter by key.
    #[must_use]
    pub fn get_parameter(&self, key: &str) -> Option<&String> {
        self.parameters.get(key)
    }
}

/// Background job contract — scheduled or run-on-startup.
///
/// Marked `#[async_trait]` because it is collected and dispatched as
/// `&'static dyn Job` via `inventory`.
#[async_trait]
pub trait Job: Send + Sync + 'static {
    /// Stable, human-readable name shown in scheduler logs.
    fn name(&self) -> &'static str;

    /// Optional human-readable description of what the job does.
    fn description(&self) -> &'static str {
        ""
    }

    /// Cron-like schedule expression for periodic execution.
    fn schedule(&self) -> &'static str;

    /// Free-form classification tags exposed to the scheduler UI.
    fn tags(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Run the job once and return its outcome.
    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult>;

    /// Whether the scheduler should consider this job for execution.
    fn enabled(&self) -> bool {
        true
    }

    /// Whether the scheduler should fire the job once at startup.
    fn run_on_startup(&self) -> bool {
        false
    }
}

inventory::collect!(&'static dyn Job);

/// Compile-time-register a `&'static dyn Job` with the inventory bus.
#[macro_export]
macro_rules! submit_job {
    ($job:expr) => {
        inventory::submit!($job as &'static dyn $crate::Job);
    };
}
