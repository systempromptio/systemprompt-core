//! The registry of background shell commands: one supervising task per command,
//! owned by the registry's task set, writing output to a log file and telling
//! the listener when it ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use harness_ids::{JobId, SessionId};
use harness_tasks::{BackgroundTasks, CancellationToken};
use parking_lot::Mutex;

use super::jobs::{JobInfo, JobListener, JobReport, JobState};
use super::process::{self, Ending, Running};
use super::select::ShellSpec;
use crate::error::ToolError;
use crate::text::ceil_boundary;

const TAIL_BYTES: usize = 16 * 1024;

#[derive(Debug)]
struct Entry {
    info: JobInfo,
    cancel: CancellationToken,
    silent: bool,
}

#[derive(Default)]
struct Inner {
    jobs: Mutex<HashMap<JobId, Entry>>,
    listener: Mutex<Option<Arc<dyn JobListener>>>,
    tasks: BackgroundTasks,
}

/// The registry of background shell commands.
///
/// Cheap to clone; clones share the same jobs. Each
/// command's supervising task belongs to the registry's [`BackgroundTasks`]:
/// its own by default, or the engine's when built with `with_tasks`, so
/// shutting that set down stops the commands too.
#[derive(Clone, Default)]
pub struct BackgroundJobs {
    inner: Arc<Inner>,
}

impl fmt::Debug for BackgroundJobs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackgroundJobs")
            .field("jobs", &self.inner.jobs.lock().len())
            .finish()
    }
}

pub(crate) struct JobStart<'a> {
    pub(crate) spec: &'a ShellSpec,
    pub(crate) command: &'a str,
    pub(crate) cwd: &'a Path,
    pub(crate) session_id: &'a SessionId,
    pub(crate) data_dir: &'a Path,
    pub(crate) timeout: Option<Duration>,
}

struct Supervised {
    id: JobId,
    running: Running,
    log: File,
    cancel: CancellationToken,
    timeout: Option<Duration>,
}

impl BackgroundJobs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tasks(tasks: BackgroundTasks) -> Self {
        let inner = Inner {
            jobs: Mutex::default(),
            listener: Mutex::default(),
            tasks,
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn set_listener(&self, listener: Arc<dyn JobListener>) {
        *self.inner.listener.lock() = Some(listener);
    }

    pub fn list(&self, session_id: &SessionId) -> Vec<JobInfo> {
        let mut jobs: Vec<JobInfo> = self
            .inner
            .jobs
            .lock()
            .values()
            .filter(|entry| &entry.info.session_id == session_id)
            .map(|entry| entry.info.clone())
            .collect();
        jobs.sort_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        jobs
    }

    pub fn get(&self, id: &JobId) -> Option<JobInfo> {
        self.inner
            .jobs
            .lock()
            .get(id)
            .map(|entry| entry.info.clone())
    }

    pub fn read(&self, id: &JobId, max_bytes: usize) -> Result<String, ToolError> {
        let path = self
            .inner
            .jobs
            .lock()
            .get(id)
            .map(|entry| entry.info.output_path.clone())
            .ok_or_else(|| ToolError::invalid(format!("there is no background job {id}")))?;
        let failed = |err: std::io::Error| {
            ToolError::io(format!("could not read the output of job {id}"), err)
        };
        let mut file = File::open(&path).map_err(failed)?;
        let length = file.metadata().map_err(failed)?.len();
        let start = length.saturating_sub(u64::try_from(max_bytes).unwrap_or(u64::MAX));
        file.seek(SeekFrom::Start(start)).map_err(failed)?;
        let mut bytes = Vec::new();
        file.take(length - start)
            .read_to_end(&mut bytes)
            .map_err(failed)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if start == 0 {
            return Ok(text);
        }
        let skip = text.find('\n').map_or(0, |at| at + 1);
        Ok(text[skip..].to_owned())
    }

    pub fn kill(&self, id: &JobId) -> bool {
        let jobs = self.inner.jobs.lock();
        match jobs.get(id) {
            Some(entry) if entry.info.state == JobState::Running => {
                entry.cancel.cancel();
                true
            },
            _ => false,
        }
    }

    pub fn kill_all(&self, session_id: &SessionId) -> usize {
        self.remove_where(|entry| &entry.info.session_id == session_id)
    }

    pub fn shutdown(&self) -> usize {
        self.remove_where(|_| true)
    }

    fn remove_where(&self, matches: impl Fn(&Entry) -> bool) -> usize {
        let mut jobs = self.inner.jobs.lock();
        let mut running = 0;
        jobs.retain(|_, entry| {
            if !matches(entry) {
                return true;
            }
            if entry.info.state == JobState::Running {
                running += 1;
                entry.silent = true;
                entry.cancel.cancel();
            }
            false
        });
        running
    }

    pub(crate) fn start(&self, request: &JobStart<'_>) -> Result<JobInfo, ToolError> {
        let id = JobId::mint();
        let dir = request.data_dir.join("jobs");
        std::fs::create_dir_all(&dir)
            .map_err(|err| ToolError::io(format!("could not create {}", dir.display()), err))?;
        let output_path = dir.join(format!("{id}.log"));
        let log = File::create(&output_path).map_err(|err| {
            ToolError::io(format!("could not create {}", output_path.display()), err)
        })?;
        let running = process::start(request.spec, request.command, request.cwd)?;
        let cancel = self.inner.tasks.child_token();
        let info = JobInfo {
            id: id.clone(),
            session_id: request.session_id.clone(),
            command: request.command.to_owned(),
            output_path,
            started_at: SystemTime::now(),
            finished_at: None,
            state: JobState::Running,
            exit_code: None,
        };
        let entry = Entry {
            info: info.clone(),
            cancel: cancel.clone(),
            silent: false,
        };
        self.inner.jobs.lock().insert(id.clone(), entry);
        tracing::info!(job = %id, session = %request.session_id, "background command started");
        let job = Supervised {
            id,
            running,
            log,
            cancel,
            timeout: request.timeout,
        };
        self.inner.tasks.spawn(
            "background shell command",
            supervise(Arc::clone(&self.inner), job),
        );
        Ok(info)
    }
}

impl Inner {
    fn finish(&self, id: &JobId, ending: Ending, tail: String) -> Option<JobReport> {
        let (state, exit_code) = match ending {
            Ending::Exited(code) => (JobState::Exited, code),
            Ending::TimedOut => (JobState::TimedOut, None),
            Ending::Cancelled => (JobState::Killed, None),
        };
        tracing::info!(job = %id, state = ?state, exit_code = ?exit_code, "background command ended");
        let mut jobs = self.jobs.lock();
        let entry = jobs.get_mut(id)?;
        entry.info.state = state;
        entry.info.exit_code = exit_code;
        entry.info.finished_at = Some(SystemTime::now());
        let report = (!entry.silent).then(|| JobReport {
            job: entry.info.clone(),
            tail,
        });
        drop(jobs);
        report
    }
}

async fn supervise(inner: Arc<Inner>, job: Supervised) {
    let Supervised {
        id,
        running,
        mut log,
        cancel,
        timeout,
    } = job;
    let mut tail = String::new();
    let ending = running
        .drive(timeout, &cancel, |chunk| {
            if let Err(err) = log.write_all(chunk.as_bytes()) {
                tracing::debug!(error = %err, "could not append to a background job log");
            }
            tail.push_str(chunk);
            if tail.len() > TAIL_BYTES * 2 {
                let cut = ceil_boundary(&tail, tail.len() - TAIL_BYTES);
                tail.drain(..cut);
            }
        })
        .await;
    if let Err(err) = log.flush() {
        tracing::debug!(error = %err, "could not flush a background job log");
    }
    drop(log);
    let report = inner.finish(&id, ending, tail);
    let listener = inner.listener.lock().clone();
    if let (Some(report), Some(listener)) = (report, listener) {
        listener.finished(&report);
    }
}
