//! Unit tests for services modules.
//!
//! Tests cover:
//! - UserAdminService (PromoteResult, DemoteResult enums)
//! - UserProviderImpl conversions
//! - UpdateUserParams
//! - ApiKeyService pure logic (prefix, name validation)
//! - DeviceCertService pure logic (fingerprint validation)

mod admin_service;
mod api_key;
mod device_cert;
mod user_provider;
mod user_provider_impl;
