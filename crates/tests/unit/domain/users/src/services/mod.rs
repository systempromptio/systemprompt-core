//! Unit tests for services modules.
//!
//! Tests cover:
//! - UserAdminService (PromoteResult, DemoteResult enums)
//! - User→AuthUser conversion
//! - UpdateUserParams
//! - ApiKeyService pure logic (prefix, name validation)
//! - DeviceCertService pure logic (fingerprint validation)

mod admin_service;
mod admin_service_db;
mod api_key;
mod api_key_db;
mod device_cert;
mod device_cert_service;
mod providers_db;
mod user_provider;
mod user_provider_impl;
mod user_service;
