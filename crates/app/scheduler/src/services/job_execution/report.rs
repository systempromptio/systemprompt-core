//! Run and batch reports for on-demand job execution, and `--param` parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::time::Instant;

use systemprompt_identifiers::JobName;

use crate::error::{SchedulerError, SchedulerResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobSelection {
    All,
    Tag(String),
    Names(Vec<JobName>),
}

#[derive(Debug, Clone)]
pub struct JobRunReport {
    pub job_name: JobName,
    pub success: bool,
    pub duration_ms: u64,
    pub message: Option<String>,
    pub items_processed: Option<u64>,
    pub items_failed: Option<u64>,
    pub lock_contended: bool,
}

#[derive(Debug, Clone, Default)]
pub struct JobBatchReport {
    pub runs: Vec<JobRunReport>,
    pub succeeded: usize,
    pub failed: usize,
    pub contended: usize,
}

pub fn parse_job_parameters(params: &[String]) -> SchedulerResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    for param in params {
        let Some((key, value)) = param.split_once('=') else {
            return Err(SchedulerError::InvalidJobParameter {
                parameter: param.clone(),
            });
        };
        map.insert(key.to_owned(), value.to_owned());
    }
    Ok(map)
}

pub(super) fn failed_report(
    job_name: &JobName,
    start: Instant,
    message: Option<String>,
) -> JobRunReport {
    JobRunReport {
        job_name: job_name.clone(),
        success: false,
        duration_ms: elapsed_ms(start),
        message,
        items_processed: None,
        items_failed: None,
        lock_contended: false,
    }
}

pub(super) fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
