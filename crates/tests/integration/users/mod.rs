//! Integration tests for the users domain.
//!
//! Tests cover:
//! - UserRepository database operations
//! - UserService business logic
//! - BannedIpRepository ban management
//! - UserAdminService admin operations

pub mod admin_service;
pub mod banned_ip_repository;
pub mod user_repository;
pub mod user_service;
