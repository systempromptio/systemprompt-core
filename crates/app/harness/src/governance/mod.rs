//! The engine's Governor for a signed-in systemprompt instance.
//!
//! [`Governance`] holds the applied instance (its projection snapshot, the
//! client that reaches it, the source of its session tokens and the extensions
//! it enabled) and implements [`harness_engine::Governor`]: every model call
//! and tool decision of the engine passes through it, and it serves the
//! instance's extension enablement. [`GovernanceExtension`] is the required
//! extension around it: its engine half observes session moments, prompts and
//! finished tools ([`GovernanceTelemetry`]), its instance half follows the
//! applied instance ([`GovernanceApplier`]).
//!
//! - Model calls go only to the instance's own gateway (`<instance>/v1`),
//!   signed with a JWT that is valid for at least a minute; a refused token is
//!   renewed once, and a second refusal is reported to the credential source.
//! - Managed MCP servers on the instance's origin get the JWT, read fresh
//!   before every request; others never do.
//! - While a manifest plugin owns governance, session, prompt and tool events
//!   are reported to the instance as hook events, fire-and-forget on an owned
//!   sender with a bounded queue, and every tool asks the instance for a
//!   verdict first. The verdict is the one place that fails open: no answer
//!   within its budget lets the tool run.
//! - Usage counters (calls forwarded, tokens in and out, last activity) feed
//!   the heartbeat.
//!
//! The sync engine and the shell drive it through [`GovernanceApplier`] (over
//! [`Governance::apply`], or [`Governance::install`] with an injected
//! [`HookApi`], and [`Governance::clear`]), [`Governance::set_device`],
//! [`Governance::applied`], [`Governance::usage`],
//! [`Governance::set_verdicts_enabled`] and [`Governance::session_bindings`].
//! Credentials live in memory only and never reach a log, an error or a
//! `Debug`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod extension;
mod governance;
mod hooks;
mod instance;
mod mcp;
mod models;
mod telemetry;
mod usage;

pub use error::GovernanceError;
pub use extension::{GovernanceApplier, GovernanceExtension};
pub use governance::{Governance, GovernanceTelemetry, InstanceParts, SessionBinding};
pub use hooks::HookApi;
pub use usage::UsageTotals;
