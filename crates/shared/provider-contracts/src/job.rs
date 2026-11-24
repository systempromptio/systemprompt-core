//! [`Job`] contract for scheduled / on-startup background jobs a member
//! contributes to the admitted schedule.
//!
//! Jobs are dispatched as `dyn Job` trait objects, so the trait uses
//! `#[async_trait]`; native `async fn` in traits is not `dyn`-compatible.
//!
//! [`JobResult::is_idle`] is a successful run that found nothing to do: zero
//! items processed, none failed, and no message. The scheduler does not record
//! such a tick, so a job opts in by returning `with_stats(0, 0)`; a job that
//! reports no stats at all is always recorded. [`Job::configured`] answers
//! whether this deployment has what the job needs (an exporter with no
//! destination answers `false`); an unconfigured job is never put on the cron
//! schedule, so it costs no ticks, but it can still be run by name. The
//! deployment map it reads carries no profile, configuration or secret: only
//! the typed deployment facts the scheduler member derives for its own jobs,
//! so a member's job that needs a deployment fact reads a value its own
//! member provides from its execution context instead.
//! [`Job::default_schedule`] is the opt-in for a job whose absence breaks
//! something (a read model it alone fills): under the scheduler's
//! `unlisted_jobs: default` it is scheduled even when the configuration does
//! not name it.
//!
//! [`Job::requires`] lists the types the job cannot run without: the scheduler
//! checks each against the job's own context before it starts the cron loop
//! and refuses boot naming the job and the type, so an unlinked or disabled
//! provider is a boot error, never a run that fails or a job that is quietly
//! dropped. A type the job reads with [`JobContext::find`] is optional by
//! definition and is not listed; an optional read never switches a
//! protection off. [`Job::missed_runs`] is what the scheduler does at start
//! with the instants a job's schedule passed while no replica ran it.
//!
//! [`Job::db_budget`] declares the statement, lock and memory limits the
//! job's run transactions carry; see [`crate::DbBudget`].
//!
//! A job reaches the host's handles through [`JobContext::get`]: the context
//! carries the kernel's open seeds (`DbPool`, `Arc<AppPaths>`, the file
//! storage, the instance id, the services view, …), every admitted member's
//! provided values and the per-run handles (`BudgetedDb`), and
//! `ctx.get::<DbPool>()?` inside `execute` either yields the pool or fails the
//! run with [`crate::ProviderError::MissingDependency`]. [`JobContext::new`]
//! withholds every sealed seed (the runtime `Config`, the secret store, the
//! signing keys, the system admin, the services root, the event router), so no
//! job or runner service reads one; the application context and the profile
//! are never in a job context. [`JobContext::for_member`] is the scheduler's
//! constructor: it withholds the same set except the sealed values the job's
//! owning member provided itself, so a member's job reads its own sealed
//! value and never another member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{Actor, ExtensionId};

use crate::dependencies::Dependencies;
use crate::dependency_error::MissingDependency;
use crate::error::ProviderResult;
use crate::state_requirement::StateRequirement;

#[derive(Debug, Clone)]
pub struct JobResult {
    pub success: bool,
    pub message: Option<String>,
    pub items_processed: Option<u64>,
    pub items_failed: Option<u64>,
    pub duration_ms: u64,
}

impl JobResult {
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

    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    #[must_use]
    pub const fn with_stats(mut self, processed: u64, failed: u64) -> Self {
        self.items_processed = Some(processed);
        self.items_failed = Some(failed);
        self
    }

    #[must_use]
    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    #[must_use]
    pub const fn is_idle(&self) -> bool {
        self.success
            && matches!(self.items_processed, Some(0))
            && matches!(self.items_failed, None | Some(0))
            && self.message.is_none()
    }

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

#[derive(Debug)]
pub struct JobContext {
    actor: Actor,
    dependencies: Dependencies,
    parameters: HashMap<String, String>,
    enforce: bool,
}

impl JobContext {
    #[must_use]
    pub fn new(actor: Actor, dependencies: Dependencies) -> Self {
        Self {
            actor,
            dependencies: dependencies.withholding_sealed(),
            parameters: HashMap::new(),
            enforce: false,
        }
    }

    #[must_use]
    pub fn for_member(actor: Actor, dependencies: Dependencies, owner: &ExtensionId) -> Self {
        Self {
            actor,
            dependencies: dependencies.withholding_sealed_except(owner),
            parameters: HashMap::new(),
            enforce: false,
        }
    }

    #[must_use]
    pub const fn enforce(&self) -> bool {
        self.enforce
    }

    #[must_use]
    pub const fn with_enforce(mut self, enforce: bool) -> Self {
        self.enforce = enforce;
        self
    }

    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    #[must_use]
    pub fn with_parameters(mut self, parameters: HashMap<String, String>) -> Self {
        self.parameters = parameters;
        self
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Result<&T, MissingDependency> {
        self.dependencies.get::<T>()
    }

    #[must_use]
    pub fn find<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.dependencies.find::<T>()
    }

    #[must_use]
    pub const fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    #[must_use]
    pub const fn parameters(&self) -> &HashMap<String, String> {
        &self.parameters
    }

    #[must_use]
    pub fn get_parameter(&self, key: &str) -> Option<&String> {
        self.parameters.get(key)
    }

    pub fn get_parameter_parsed<T: std::str::FromStr>(
        &self,
        key: &str,
    ) -> Result<Option<T>, crate::ProviderError>
    where
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.parameters
            .get(key)
            .map(|value| {
                value
                    .parse()
                    .map_err(|e| crate::ProviderError::InvalidParameter {
                        key: key.to_owned(),
                        value: value.clone(),
                        source: Box::new(e),
                    })
            })
            .transpose()
    }
}

/// Where a job runs when the scheduler has several replicas.
///
/// `Cluster` jobs run once per scheduled instant across the whole deployment
/// (one replica's claim of the instant's run row wins). `Node` jobs run once
/// per instant on every replica, because their effect is local to the process
/// or its filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobScope {
    #[default]
    Cluster,
    Node,
}

/// What the scheduler does at start with the instants a job's schedule passed
/// since the last instant any replica recorded for it.
///
/// `Skip` runs none of them and waits for the next instant. `RunOnce` runs
/// the latest missed instant once. `CatchUp { max }` runs up to `max` of the
/// most recent missed instants, oldest first. A job with no recorded instant
/// has missed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MissedRunPolicy {
    #[default]
    Skip,
    RunOnce,
    CatchUp {
        max: u32,
    },
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

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult>;

    fn enabled(&self) -> bool {
        true
    }

    fn configured(&self, _deployment: &Dependencies) -> bool {
        true
    }

    fn schedulable(&self) -> bool {
        true
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    fn default_schedule(&self) -> Option<&'static str> {
        None
    }

    fn db_budget(&self) -> Option<crate::DbBudget> {
        None
    }

    fn requires(&self) -> Vec<StateRequirement> {
        Vec::new()
    }

    fn missed_runs(&self) -> MissedRunPolicy {
        MissedRunPolicy::Skip
    }
}
