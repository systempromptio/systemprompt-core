//! `systemprompt-agent-runs` — durable, leased, governed coding-agent runs
//! across hosts.
//!
//! A product (an agent farm, a workflow engine) queues a [`RunRequest`] as a
//! `agent_runs` row. A claim loop running inside an extension's
//! `RunnerService` claims it, prepares a private [`RunLayout`], issues the
//! run ephemeral credentials so every gateway call is attributed, spawns the
//! host named by [`HostKind`] as an owned supervised subprocess and decodes
//! its event stream into [`RunEvent`]s, ending in a [`RunOutcome`].
//!
//! Hosts implement [`HeadlessHost`]; [`host_for`] resolves one, and `OpenCode`
//! and Hermes resolve to [`Execution::Unsupported`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[doc(hidden)]
pub use ::inventory as __inventory;

mod macros;

pub mod a2a;
pub mod credentials;
pub mod error;
pub mod event;
pub mod execute;
pub mod extension;
pub mod governance;
pub mod host;
pub mod launcher;
pub mod layout;
pub mod lease;
pub mod notify;
pub mod origin;
pub mod outcome;
pub mod repository;
pub mod request;
pub mod retention;
pub mod runner;
pub mod service;

pub use a2a::{TaskUpdate, task_events};
pub use credentials::{IssueRunCredentials, RunCredentials};
pub use error::{AgentRunsError, AgentRunsResult};
pub use event::{HookDecision, RunEvent, RunEventKind, RunEventPayload, RunnerLevel};
pub use execute::{
    DiffEntry, DiffKind, DiffSummary, EventSink, ExecuteParams, RunReport, execute,
    execute_with_host,
};
pub use extension::AgentRunsExtension;
pub use host::{
    Execution, ExitInfo, HeadlessHost, HostCapabilities, HostEvent, HostInvocation, PrepareContext,
    VersionRequirement, host_for,
};
pub use launcher::RepositoryRunLauncher;
pub use layout::RunLayout;
pub use outcome::{ErrorClass, RunOutcome, RunStatus};
pub use repository::{AgentRunRepository, AgentRunRow};
pub use request::{HarnessRef, HostKind, McpServerSpec, PermissionMode, RunRequest, TrafficClass};
pub use runner::{A2aRunHandler, A2aRunnerConfig, A2aRunnerService, AGENT_RUN_ORIGIN_KIND};
