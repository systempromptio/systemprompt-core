//! `systemprompt-headless` — host-generic headless coding-agent executor.
//!
//! A product (an agent farm, a workflow engine) queues a [`RunRequest`] as a
//! `headless_runs` row. A claim loop running inside an extension's
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

mod macros;

pub mod credentials;
pub mod error;
pub mod event;
pub mod execute;
pub mod extension;
pub mod governance;
pub mod host;
pub mod layout;
pub mod lease;
pub mod outcome;
pub mod repository;
pub mod request;
pub mod service;

pub use credentials::{IssueRunCredentials, RunCredentials};
pub use error::{HeadlessError, HeadlessResult};
pub use event::{HookDecision, RunEvent, RunEventKind, RunEventPayload, RunnerLevel};
pub use execute::{EventSink, execute};
pub use execute::{DiffEntry, DiffKind, DiffSummary, ExecuteParams, RunReport, execute_with_host};
pub use extension::HeadlessExtension;
pub use host::{
    ExitInfo, Execution, HeadlessHost, HostCapabilities, HostEvent, HostInvocation, PrepareContext,
    host_for,
};
pub use layout::RunLayout;
pub use outcome::{ErrorClass, RunOutcome, RunStatus};
pub use repository::{HeadlessRunRepository, HeadlessRunRow};
pub use request::{HostKind, McpServerSpec, PermissionMode, RunRequest};
