//! Live sync of the active systemprompt account.
//!
//! [`SyncEngine`] is one per process and a cheap-clone handle over owned
//! background tasks. It follows the accounts: a loop runs for the active
//! signed-in account only, polls its signed manifest, accepts it, downloads the
//! plugin bundle files it names, projects and applies it when its content
//! changed, hands the result and the extensions its manifest enabled to the
//! extensions' manifest appliers (governance among them), then delivers the
//! skills' runtime files and their receipts. A heartbeat reports this install
//! as online. Every failure is a [`SyncStatus`](sp_protocol::view::SyncStatus),
//! never a panic or an error returned to a caller.
//!
//! The engine is generic over the instance, the accounts and the apply target
//! ([`ports`], [`target`]); [`SyncEngine::start`] wires the live ones and
//! [`SyncEngine::launch`] takes any.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod acceptance;
mod account_loop;
/// Plugin bundle files, cached by digest in memory and on disk.
#[doc(hidden)]
pub mod bundles;
/// Poll intervals, jitter and every backoff, pure over an injected clock and
/// jitter source.
pub mod cadence;
mod engine;
mod error;
pub mod heartbeat;
pub mod outbox;
/// Which poll runs next, with which options, and the wait after it.
pub mod planner;
/// One poll of one account.
#[doc(hidden)]
pub mod poll;
pub mod ports;
/// Device enrolment, installation plans, materialized runtime files, receipts
/// and bindings.
#[doc(hidden)]
pub mod runtime;
mod supervisor;
pub mod target;

pub use engine::{Launch, SyncDeps, SyncEngine};
pub use error::SyncError;
pub use heartbeat::{BRIDGE_PROTOCOL_VERSION, HeartbeatState};
