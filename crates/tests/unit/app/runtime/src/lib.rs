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

#![allow(clippy::all)]

mod context;
mod installation;
mod registry;
mod span;
mod startup_validation;
mod validation;
mod wellknown;
