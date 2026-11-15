//! Top-level CLI command groups.
//!
//! Each submodule owns one command domain — [`admin`], [`analytics`],
//! [`core`], [`infrastructure`], [`plugins`], [`web`], and the build tooling
//! in [`build`] — and exposes its own clap subcommand tree. Groups owned by a
//! capability's entry crate (such as `cloud`) register themselves through the
//! kernel instead.
//! The [`shared`] module holds helpers used across those groups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin;
pub mod analytics;
pub mod build;
pub mod core;
pub mod infrastructure;
pub mod plugins;
pub mod shared;
pub mod web;
