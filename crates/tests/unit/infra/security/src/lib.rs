//! Unit tests for systemprompt-security crate.

#![allow(clippy::all)]

#[cfg(test)]
mod acl_glob;
#[cfg(test)]
mod at_rest;
#[cfg(test)]
mod auth_validation;
#[cfg(test)]
mod authz_entity_ref;
#[cfg(test)]
mod authz_repository;
#[cfg(test)]
mod error_display;
#[cfg(test)]
mod extraction;
#[cfg(test)]
mod hook_token_typed_ids;
#[cfg(test)]
mod jwks_fetch;
#[cfg(test)]
mod jwt_extract;
#[cfg(test)]
mod jwt_validate;
#[cfg(test)]
mod manifest_signing_jcs;
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
mod policy_types;
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
