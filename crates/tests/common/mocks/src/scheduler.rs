use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_traits::scheduler::{JobInfo, JobStatus, JobTrigger, SchedulerError};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct TriggerCall {
    pub job_name: String,
}

pub struct MockJobTrigger {
    trigger_calls: Arc<Mutex<Vec<TriggerCall>>>,
    trigger_errors: HashMap<String, String>,
    job_statuses: HashMap<String, JobInfo>,
    is_running: bool,
}

impl MockJobTrigger {
    #[must_use]
    pub fn new() -> Self {
        Self {
            trigger_calls: Arc::new(Mutex::new(Vec::new())),
            trigger_errors: HashMap::new(),
            job_statuses: HashMap::new(),
            is_running: true,
        }
    }

    #[must_use]
    pub fn with_trigger_error(mut self, job_name: impl Into<String>, err: impl Into<String>) -> Self {
        self.trigger_errors.insert(job_name.into(), err.into());
        self
    }

    #[must_use]
    pub fn with_job_status(mut self, name: impl Into<String>, info: JobInfo) -> Self {
        self.job_statuses.insert(name.into(), info);
        self
    }

    #[must_use]
    pub fn with_running(mut self, running: bool) -> Self {
        self.is_running = running;
        self
    }

    pub async fn trigger_calls(&self) -> Vec<TriggerCall> {
        self.trigger_calls.lock().await.clone()
    }

    pub fn make_job_info(name: impl Into<String>, status: JobStatus) -> JobInfo {
        JobInfo {
            name: name.into(),
            status,
            last_run: None,
            next_run: None,
            run_count: 0,
            last_error: None,
        }
    }
}

impl Default for MockJobTrigger {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JobTrigger for MockJobTrigger {
    async fn trigger_job(&self, job_name: &str) -> Result<(), SchedulerError> {
        self.trigger_calls.lock().await.push(TriggerCall {
            job_name: job_name.to_string(),
        });

        if let Some(err) = self.trigger_errors.get(job_name) {
            return Err(SchedulerError::ExecutionFailed(err.clone()));
        }

        Ok(())
    }

    async fn get_job_status(&self, job_name: &str) -> Result<JobInfo, SchedulerError> {
        self.job_statuses
            .get(job_name)
            .cloned()
            .ok_or_else(|| SchedulerError::JobNotFound(job_name.to_string()))
    }

    async fn list_jobs(&self) -> Result<Vec<JobInfo>, SchedulerError> {
        Ok(self.job_statuses.values().cloned().collect())
    }

    async fn is_running(&self) -> bool {
        self.is_running
    }
}
