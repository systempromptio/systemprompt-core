//! Unit tests for systemprompt-security crate.

#![allow(clippy::all)]

#[cfg(test)]
mod acl_glob;
#[cfg(test)]
mod at_rest;
#[cfg(test)]
mod error_display;
#[cfg(test)]
mod extraction;
#[cfg(test)]
mod hook_token_typed_ids;
#[cfg(test)]
mod jwt_extract;
#[cfg(test)]
mod jwt_validate;
#[cfg(test)]
mod manifest_signing_jcs;
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
