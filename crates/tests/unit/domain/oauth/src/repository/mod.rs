//! DB-backed tests for the OAuth persistence layer.
//!
//! Each test guards on `fixture_database_url()` and returns early when no
//! `DATABASE_URL` is configured, so the suite is a no-op without a database.
//! The gateway runs these against a fresh, freshly-migrated Postgres instance.

mod auth_code;
mod bridge_host_prefs;
mod bridge_session;
mod cleanup;
mod client_cleanup;
mod client_crud;
mod client_relations;
mod exchange_code;
mod jti_revocation;
mod oauth_facade;
mod refresh_token;
mod scopes;
mod setup_token;
mod state_binding;
mod user;
mod webauthn;
