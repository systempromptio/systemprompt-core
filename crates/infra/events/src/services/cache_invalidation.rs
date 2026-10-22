//! Cross-replica cache invalidation over Postgres `NOTIFY`.
//!
//! Statement triggers on the tables behind the in-memory caches (gateway
//! policies, access-control rules, JTI revocations) call
//! `pg_notify('systemprompt_cache', '<domain>')` inside the writing
//! transaction, so a notification is sent exactly when the write commits and
//! whichever replica or tool made it. The [`crate::PostgresEventBridge`]
//! listens on [`CACHE_INVALIDATION_CHANNEL`] beside the event outbox and
//! advances the matching [`CacheDomain`] epoch (a bare domain label, or
//! `config:<plane>` from a config plane's version trigger) in the
//! [`CacheEpochs`] handle
//! it was built with; each cache drops entries
//! filled under an older epoch. There is no outbox: a lost notification
//! leaves the cache's TTL as the bound, and the bridge advances every domain
//! whenever its listener (re)connects.
//!
//! The channel needs a session-level `LISTEN`, so it does not work through a
//! transaction-pooling proxy — the same constraint as the event outbox. Point
//! `database_session_url` at a direct, session-capable connection (the
//! primary, or a pooler in session mode) whenever `database_url` goes through
//! `PgBouncer` in transaction mode: a `LISTEN` there can succeed and then never
//! deliver. The relay marks the shared [`CacheEpochs`] live only while its
//! listener is connected; a process with no relay, or one between
//! reconnects, is not live, and the JTI cache then stops trusting cached
//! "not revoked" answers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{CacheDomain, CacheEpochs};
use tracing::debug;

pub const CACHE_INVALIDATION_CHANNEL: &str = "systemprompt_cache";
pub const CACHE_INVALIDATIONS_RECEIVED_TOTAL: &str = "cache_invalidations_received_total";

pub fn receive_cache_invalidation(epochs: &CacheEpochs, payload: &str) -> Option<CacheDomain> {
    let Some(domain) = CacheDomain::from_notification(payload.trim()) else {
        debug!(payload, "cache invalidation: unknown domain; ignoring");
        return None;
    };
    let epoch = epochs.advance(domain);
    metrics::counter!(CACHE_INVALIDATIONS_RECEIVED_TOTAL, "domain" => domain.label()).increment(1);
    debug!(
        domain = domain.label(),
        epoch, "cache invalidation received"
    );
    Some(domain)
}

pub fn invalidate_all_cache_domains(epochs: &CacheEpochs) {
    epochs.advance_all();
}
