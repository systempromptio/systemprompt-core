//! `systemprompt-cloud-entry` — the cloud entry surface.
//!
//! The `cli` feature carries the `cloud` command group ([`cli`]): tenant
//! sign-in, project scaffolding, tenants, profiles, deploy, backup, doctor,
//! status, fleet operations, the Dockerfile and database-to-code sync.
//! [`CloudCliExtension`] declares the group under the reserved `cloud` path,
//! so a composition with this member built with `cli` gains
//! `systemprompt cloud …`; one without it has no `cloud` command. The cloud API client, credentials and tenant store are
//! `systemprompt-cloud`. The `http` feature carries no routes: the cloud
//! capability has no HTTP surface of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;

pub use extension::CloudCliExtension;
