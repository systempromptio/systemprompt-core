//! `systemprompt-machine` — machine-level install and policy.
//!
//! Scheduler entries, elevation and the locations a managed configuration is
//! read from. Each concern is one typed contract with a Windows, a macOS and a
//! Linux implementation behind it in the `platform` module. No other module of
//! the crate names a target system, so a system without an implementation does
//! not compile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod elevation;
mod platform;
pub mod policy;
mod quote;
pub mod scheduler;
