//! Unit tests for repository modules.
//!
//! Tests cover:
//! - BanDuration enum and methods
//! - BanIpParams builder pattern
//! - BanIpWithMetadataParams builder pattern
//! - BannedIp struct
//! - MergeResult struct

mod banned_ip;
mod banned_ip_db;
mod device_cert;
mod federated_identity;
mod federated_identity_naming;
mod user;
mod user_queries_db;
mod user_sessions_db;
