//! Integration tests for the gateway audit / `ContextId` derivation surface.
//!
//! The `context_id_*` and `payload_*` modules are pure and do not require a
//! database. The `audit_concurrency` module exercises `GatewayAudit::new`
//! against a live Postgres and requires `DATABASE_URL` pointing at a fully
//! migrated test database (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod audit_concurrency;
#[cfg(test)]
mod context_id_derivation;
#[cfg(test)]
mod payload_truncation;
#[cfg(test)]
mod stream_tap_pipeline;
#[cfg(test)]
mod support;
