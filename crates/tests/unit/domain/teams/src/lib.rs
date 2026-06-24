//! Unit tests for the `systemprompt-teams` crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/teams/src/<module>.rs`
//! - Test:   `crates/tests/unit/domain/teams/src/<module>.rs`
//!
//! Coverage: Bot Framework activity-token validation (RS256 against a locally
//! minted keypair — no network), declarative app config + manifest projection,
//! Adaptive Card rendering, inbound activity normalization, the outbound
//! reply-URL builder, and the outbound token-cache skew/expiry arithmetic. All
//! pure-logic — no database, no live network.

#![allow(clippy::all)]

#[cfg(test)]
mod activities;
#[cfg(test)]
mod auth;
#[cfg(test)]
mod auth_jwks;
#[cfg(test)]
mod cards;
#[cfg(test)]
mod client;
#[cfg(test)]
mod client_reply;
#[cfg(test)]
mod config;
#[cfg(test)]
mod token;
