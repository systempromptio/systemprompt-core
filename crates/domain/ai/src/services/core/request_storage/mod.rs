//! Audit-row persistence for AI requests.
//!
//! [`RequestStorage::store`] is invoked from every `AiService` path that
//! produces an `AiResponse`. The primary `ai_requests` insert returns
//! `Result<(), AiError>` so callers can decide whether an audit failure
//! should fail the originating request. Secondary writes (per-message rows,
//! per-tool-call rows, session usage, analytics events) are best-effort:
//! each logs at error level on failure and never aborts the others.
//!
//! Persistence is fully synchronous — there is no detached `tokio::spawn`.
//! A storage error reaches the caller in the same task that originated the
//! request, which is the only design under which a broken trigger or a
//! schema drift becomes visible in test and production alike.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod record_builder;
mod storage;
mod writes;

pub use storage::{RequestStorage, StoreParams};
