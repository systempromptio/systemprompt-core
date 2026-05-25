//! Integration tests for the federated JWKS plane in
//! `systemprompt-security` — kid-rotation, TTL expiry, LRU eviction,
//! algorithm enforcement, and the unknown-kid DoS guard. These tests
//! stand up a real `wiremock` HTTP JWKS endpoint, so they exercise the
//! cache + network paths that unit tests cannot.

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
