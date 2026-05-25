//! Integration tests for `systemprompt-sync` — exercises the cloud sync
//! surface that pure-unit tests cannot: real `wiremock`-backed
//! `SyncApiClient` HTTP flows (RFC 8693 token exchange, upload retry +
//! idempotent recovery, cross-tenant guard) and real-Postgres
//! `AccessControlIngestionService` flows (atomic rename, deny-overrides
//! ordering, transactional rollback on validation failure).
//!
//! The DB-bearing tests early-skip when `DATABASE_URL` is absent so the
//! suite still passes in environments without a Postgres fixture.

#[cfg(test)]
mod support;

#[cfg(test)]
mod token_exchange_tests;

#[cfg(test)]
mod upload_recovery_tests;

#[cfg(test)]
mod cross_tenant_tests;

#[cfg(test)]
mod acl_atomic_tests;

#[cfg(test)]
mod acl_resolver_tests;

#[cfg(test)]
mod acl_rollback_tests;
