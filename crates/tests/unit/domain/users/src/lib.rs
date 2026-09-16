//! Unit tests for systemprompt-core-users crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/users/src/models/mod.rs`
//! - Test: `crates/tests/unit/domain/users/src/models.rs`
//!
//! Tests cover:
//! - UserStatus, UserRole enums and helper methods
//! - User, UserActivity, UserWithSessions, UserSession structs
//! - UserError enum and error handling
//! - BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp
//! - PromoteResult, DemoteResult enums
//! - UpdateUserParams struct
//! - User to AuthUser conversion
//! - CleanupAnonymousUsersJob (trait methods only)

#[cfg(test)]
mod error;

#[cfg(test)]
mod extension;

#[cfg(test)]
mod jobs;

#[cfg(test)]
mod jobs_db;

#[cfg(test)]
mod models;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;

#[cfg(test)]
mod device_cert_reuse;

#[cfg(test)]
mod authoritative_reads_db;
#[cfg(test)]
mod session_mutations;
#[cfg(test)]
mod session_queries;
#[cfg(test)]
mod session_support;

#[cfg(test)]
mod ai_session_provider;
#[cfg(test)]
mod session_provider;

#[cfg(test)]
mod privacy_fixture;
