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
mod device_cert_service;
mod user_provider;
mod user_provider_impl;
mod user_service;
