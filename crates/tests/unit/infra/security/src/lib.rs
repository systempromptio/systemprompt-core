//! Unit tests for systemprompt-security crate.

#![allow(clippy::all)]

#[cfg(test)]
mod acl_glob;
#[cfg(test)]
mod at_rest;
#[cfg(test)]
mod auth_validation;
#[cfg(test)]
mod authz_bootstrap;
#[cfg(test)]
mod authz_chain_cache;
#[cfg(test)]
mod authz_entity_ref;
#[cfg(test)]
mod authz_repository;
#[cfg(test)]
mod authz_subject_ref;
#[cfg(test)]
mod credential;
#[cfg(test)]
mod error_display;
#[cfg(test)]
mod extraction;
#[cfg(test)]
mod governance_plane;
#[cfg(test)]
mod governance_plane_unapplied;
#[cfg(test)]
mod hook_token_typed_ids;
#[cfg(test)]
mod jwks_fetch;
#[cfg(test)]
mod jwks_shapes;
#[cfg(test)]
mod jwt_extract;
#[cfg(test)]
mod jwt_validate;
#[cfg(test)]
mod manifest_signing_jcs;
#[cfg(test)]
mod manifest_signing_keys;
#[cfg(test)]
mod oauth_wire;
#[cfg(test)]
mod policy_approval;
#[cfg(test)]
mod policy_audit;
#[cfg(test)]
mod policy_builtin_config;
#[cfg(test)]
mod policy_builtins;
#[cfg(test)]
mod policy_config;
#[cfg(test)]
mod policy_engine;
#[cfg(test)]
mod policy_governed;
#[cfg(test)]
mod policy_prompt_governance;
#[cfg(test)]
mod policy_rate_window_db;
#[cfg(test)]
mod policy_role_gate;
#[cfg(test)]
mod policy_secret_signatures;
#[cfg(test)]
mod policy_types;
#[cfg(test)]
mod policy_warn_mode;
#[cfg(test)]
mod rs256_cutover;
#[cfg(test)]
mod services;
#[cfg(test)]
mod session_generator;
#[cfg(test)]
mod signing_key_independence;
#[cfg(test)]
mod signing_key_pem_roundtrip;
#[cfg(test)]
mod token_authority;

#[cfg(test)]
mod connector_grant;
#[cfg(test)]
mod policy_recovery;

#[cfg(test)]
pub(crate) fn offline_rate_windows() -> systemprompt_governance::policy::RateWindowRepository {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_lazy("postgres://offline:offline@127.0.0.1:1/offline")
        .unwrap();
    systemprompt_governance::policy::RateWindowRepository::new(pool)
}
