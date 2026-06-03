//! Integration tests for `systemprompt-security`.
//!
//! The federated JWKS plane (kid-rotation, TTL expiry, LRU eviction,
//! algorithm enforcement, the unknown-kid DoS guard) stands up a real
//! `wiremock` HTTP JWKS endpoint. The marketplace-access ingestion suite
//! projects declarative `access` blocks into the two-table authz schema
//! against a real Postgres instance via `DATABASE_URL`.

#[cfg(test)]
mod support;

#[cfg(test)]
mod kid_rotation_tests;

#[cfg(test)]
mod ttl_expiry_tests;

#[cfg(test)]
mod lru_eviction_tests;

#[cfg(test)]
mod algorithm_rejection_tests;

#[cfg(test)]
mod revoked_kid_tests;

#[cfg(test)]
mod dos_guard_tests;

#[cfg(test)]
mod authz_extension_path_tests;

#[cfg(test)]
mod marketplace_ingestion_tests;

#[cfg(test)]
mod gateway_reconcile_tests;
