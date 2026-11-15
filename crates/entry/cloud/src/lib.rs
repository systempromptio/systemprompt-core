//! `systemprompt-cloud-entry` — the cloud entry surface.
//!
//! The `cli` feature carries the `cloud` command group ([`cli`]): tenant
//! sign-in, project scaffolding, tenants, profiles, deploy, backup, doctor,
//! status, fleet operations, the Dockerfile and database-to-code sync. The
//! group registers itself with the CLI kernel, so a binary that links this
//! crate with `cli` gains `systemprompt cloud …`; one that does not has no
//! `cloud` command. The cloud API client, credentials and tenant store are
//! `systemprompt-cloud`. The `http` feature carries no routes: the cloud
//! capability has no HTTP surface of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "cli")]
pub mod cli;
