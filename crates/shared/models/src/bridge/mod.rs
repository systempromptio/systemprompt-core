//! Bridge wire-format models.
//!
//! Types in this module are the on-the-wire shapes shared between the
//! gateway HTTP server (which serves `/v1/bridge/*` endpoints from
//! `crates/entry/api`) and the desktop bridge client (which lives at
//! `bin/bridge` and consumes those endpoints). Keeping a single typed
//! definition here prevents the two sides from drifting — the API
//! handler emits exactly what the bridge deserialises.
//!
//! # Modules
//!
//! - [`manifest`] — the signed manifest envelope and its sub-entries (plugins,
//!   skills, agents, managed MCP servers, user info).
//! - [`plugin_bundle`] — the `.claude-plugin/plugin.json` manifest shape and
//!   the well-formedness predicate shared by every plugin-bundle
//!   producer/consumer.
//! - [`manifest_version`] — the parsed `<rfc3339>-<hex>` version identifier
//!   carried inside every manifest.
//! - [`ids`] — typed newtypes for manifest-scoped identifiers (plugin id,
//!   sha256 digest, signature, tool policy, …) so wire fields carry their
//!   semantics through every layer.
//! - [`profile`] — the `/v1/bridge/profile` payload (gateway base url, auth
//!   scheme, advertised models, per-provider health) and its single builder.
//!
//! Signing, signature verification, and manifest construction
//! (builders) deliberately live in the bridge crate alongside the
//! gateway client — they pull in `ed25519-dalek` and `serde_jcs` which
//! are not appropriate dependencies for this foundation crate.

pub mod ids;
pub mod manifest;
pub mod manifest_version;
pub mod plugin_bundle;
pub mod profile;
