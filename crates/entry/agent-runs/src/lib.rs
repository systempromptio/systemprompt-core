//! `systemprompt-agent-runs-entry` — the HTTP and CLI surface of the
//! agent-runs capability.
//!
//! The `http` feature carries `/api/v1/agent-runs` ([`http`]): the routes
//! that queue, read, cancel and stream a caller's headless runs, declared
//! through the `agent_runs_api` extension and mounted by the API server with
//! the user gate and the `agent_runs` rate limit. The run queue, executor and
//! claim loop are `systemprompt-agent-runs`; its extension puts the run
//! repository and the A2A run launcher into the extension state, which these
//! routes and the `admin agents run` child host read.
//!
//! The `cli` feature registers the capability's link-time seam
//! (`agent_run_origins`) with the CLI's seam listing ([`cli`]). The
//! `admin agents run` command itself belongs to the agents capability.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "cli")]
pub mod cli;
