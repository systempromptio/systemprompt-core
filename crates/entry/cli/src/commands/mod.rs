//! Top-level CLI command groups.
//!
//! Each submodule owns one command domain — [`admin`], [`analytics`],
//! [`cloud`], [`core`], [`infrastructure`], [`plugins`], [`web`], and the
//! build tooling in [`build`] — and exposes its own clap subcommand tree.
//! The private `shared` module holds helpers used across those groups.

pub mod admin;
pub mod analytics;
pub mod build;
pub mod cloud;
pub mod core;
pub mod infrastructure;
pub mod plugins;
mod shared;
pub mod web;
