//! Integration tests for the `systemprompt-cloud` crate.
//!
//! These tests target the tenant-isolation surface of the cloud crate:
//! the multi-tenant CLI session store, the persistent tenant registry,
//! and the credential store. They run fully offline against temporary
//! directories — no real `~/.systemprompt/` is touched and no network
//! calls are issued.
//!
//! ## What "tenant" means here
//!
//! The cloud crate models *cloud-deployment* tenants: each `StoredTenant`
//! is a distinct deployed instance on the systemprompt.io control plane,
//! with its own `app_id`, `hostname`, `database_url`, and (server-side)
//! `oauth_at_rest_pepper`. Per-tenant secrets are fetched from a
//! per-tenant `secrets_url` and are never co-mingled client-side; the
//! tests in [`pepper_boundary_tests`] document that architectural
//! invariant.

#[cfg(test)]
mod concurrency_tests;

#[cfg(test)]
mod context_switch_tests;

#[cfg(test)]
mod credential_rotation_tests;

#[cfg(test)]
mod cross_tenant_token_tests;

#[cfg(test)]
mod pepper_boundary_tests;

#[cfg(test)]
mod tenant_deletion_tests;

#[cfg(test)]
mod support;
