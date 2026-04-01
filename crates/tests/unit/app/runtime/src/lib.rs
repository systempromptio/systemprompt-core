//! Unit tests for systemprompt-runtime crate
//!
//! Tests cover:
//! - AppContext and AppContextBuilder initialization and accessors
//! - ModuleApiRegistry registration, lookup, and categorization
//! - WellKnownMetadata creation and lookup
//! - Request span creation with various context configurations
//! - Module installation path resolution
//! - Database path validation
//! - StartupValidator domain registration and validation
//! - DatabaseContext creation and pool management

#![allow(clippy::all)]

#[cfg(test)]
mod context;
#[cfg(test)]
mod database_context;
#[cfg(test)]
mod installation;
#[cfg(test)]
mod registry;
#[cfg(test)]
#[cfg(test)]
mod startup_validation;
#[cfg(test)]
mod validation;
#[cfg(test)]
mod wellknown;
