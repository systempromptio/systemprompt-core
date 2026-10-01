//! DB-backed tests for the OAuth persistence layer.
//!
//! Each test takes its database from `test_db_pool()` and fails when no
//! `DATABASE_URL` is configured. The gateway runs these against a fresh,
//! freshly-migrated Postgres instance.

mod auth_code;
mod authoritative_reads_db;
mod bridge_host_prefs;
mod bridge_session;
mod client_crud;
mod client_last_used;
mod client_relations;
mod exchange_code;
mod expiry_cleanup;
mod id_jag_replay;
mod jti_revocation;
mod oauth_contact_atomicity;
mod oauth_facade;
mod refresh_token;
mod scopes;
mod setup_token;
mod state_binding;
mod webauthn;
mod webauthn_corrupt_db;
