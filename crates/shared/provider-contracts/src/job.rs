use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct JobResult {
    pub success: bool,
    pub message: Option<String>,
    pub items_processed: Option<u64>,
    pub items_failed: Option<u64>,
    pub duration_ms: u64,
}

impl JobResult {
    pub const fn success() -> Self {
        Self {
            success: true,
            message: None,
            items_processed: None,
            items_failed: None,
            duration_ms: 0,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub const fn with_stats(mut self, processed: u64, failed: u64) -> Self {
        self.items_processed = Some(processed);
        self.items_failed = Some(failed);
        self
    }

    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

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

pub struct JobContext {
    db_pool: Arc<dyn std::any::Any + Send + Sync>,
    app_context: Arc<dyn std::any::Any + Send + Sync>,
    parameters: HashMap<String, String>,
}

impl std::fmt::Debug for JobContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobContext")
            .field("db_pool", &"<type-erased>")
            .field("app_context", &"<type-erased>")
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl JobContext {
    pub fn new(
        db_pool: Arc<dyn std::any::Any + Send + Sync>,
        app_context: Arc<dyn std::any::Any + Send + Sync>,
    ) -> Self {
        Self {
            db_pool,
            app_context,
            parameters: HashMap::new(),
        }
    }

    pub fn with_parameters(mut self, parameters: HashMap<String, String>) -> Self {
        self.parameters = parameters;
        self
    }

    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.as_ref().downcast_ref::<T>()
    }

    pub fn app_context<T: 'static>(&self) -> Option<&T> {
        self.app_context.as_ref().downcast_ref::<T>()
    }

    pub fn db_pool_arc(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::clone(&self.db_pool)
    }

    pub fn app_context_arc(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::clone(&self.app_context)
    }

    pub const fn parameters(&self) -> &HashMap<String, String> {
        &self.parameters
    }

    pub fn get_parameter(&self, key: &str) -> Option<&String> {
        self.parameters.get(key)
    }
}

#[async_trait]
pub trait Job: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    fn schedule(&self) -> &'static str;

    fn tags(&self) -> Vec<&'static str> {
        vec![]
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult>;

    fn enabled(&self) -> bool {
        true
    }

    fn run_on_startup(&self) -> bool {
        false
    }
}

inventory::collect!(&'static dyn Job);

#[macro_export]
macro_rules! submit_job {
    ($job:expr) => {
        inventory::submit!($job as &'static dyn $crate::Job);
    };
}
