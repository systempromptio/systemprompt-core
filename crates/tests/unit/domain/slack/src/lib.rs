//! Unit tests for the `systemprompt-slack` crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/slack/src/<module>.rs`
//! - Test:   `crates/tests/unit/domain/slack/src/<module>.rs`
//!
//! Coverage: signature verification (Slack's `v0` HMAC scheme), declarative
//! app config + manifest projection, Block Kit rendering, and inbound payload
//! normalization. All pure-logic — no database or network.

#![allow(clippy::all)]

#[cfg(test)]
mod blockkit;
#[cfg(test)]
mod config;
#[cfg(test)]
mod events;
#[cfg(test)]
mod signature;
