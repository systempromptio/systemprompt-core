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
mod jobs;

#[cfg(test)]
mod models;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;
