//! Integration tests for `systemprompt-runtime`.
//!
//! Exercises `AppContext` context loaders, `StartupValidator::validate`,
//! and the validation-report display helpers end-to-end. These tests
//! deliberately stay clear of the global `ProfileBootstrap` / `Config`
//! `OnceLock`s — they construct fresh `Config` values per test so the
//! validator's early-bail (services-config) path runs.

#![allow(clippy::all)]

#[cfg(test)]
mod config_loaders;
#[cfg(test)]
mod display;
#[cfg(test)]
mod validate_database_path;
#[cfg(test)]
mod validator;
#[cfg(test)]
mod app_context_traits;
