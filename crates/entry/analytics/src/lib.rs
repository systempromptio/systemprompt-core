//! `systemprompt-analytics-entry` — the analytics entry surface.
//!
//! The `http` feature carries the capability's routes ([`http`]): event
//! ingestion and the live analytics stream under `/api/v1/analytics`, behind
//! the admin gate, and engagement ingestion under `/track/engagement`, which
//! is public. [`extension::AnalyticsHttpExtension`] (`analytics-http`)
//! declares both through `Extension::routers`, so the API server mounts them
//! without naming this crate.
//!
//! The `cli` feature carries the capability's commands ([`cli`]): the
//! top-level `analytics` group (overview, conversations, agents, tools,
//! requests, sessions, content, traffic, costs), registered with the CLI
//! kernel. With `content-commands` as well, it also registers the
//! `core content popular` leaf under the `core content` group of
//! `systemprompt-content-entry`; a build without that group must leave the
//! feature off, or the kernel refuses the leaf's unknown parent.
//!
//! The analytics repositories, the `GeoIP` reader and the request-analytics
//! middleware are composed by the kernel, not here: the session store they
//! carry sits on the authentication path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "http")]
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
