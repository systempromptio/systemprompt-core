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
mod builder_extra;
#[cfg(test)]
mod context;
#[cfg(test)]
mod context_loaders_extra;
#[cfg(test)]
mod database_context;
#[cfg(test)]
mod display_tests;
#[cfg(test)]
mod files_validator_load;
#[cfg(test)]
mod files_validator_tests;
#[cfg(test)]
#[cfg(test)]
mod registry;
#[cfg(test)]
mod span_tests;
#[cfg(test)]
mod startup_validation;
#[cfg(test)]
mod validate_database_path;
#[cfg(test)]
mod validation;
#[cfg(test)]
mod validation_report_extended;
#[cfg(test)]
mod wellknown;
