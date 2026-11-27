//! Request and answer bodies of every route the harness calls on an instance.
//!
//! Answers tolerate unknown fields. Requests that carry a credential borrow it
//! as `&str` from the secret's `expose()` and implement `Debug` by hand, so a
//! request body never prints a secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Sign-in and credential bodies.
pub mod auth;
/// The Brain: facts, proposals and the memory headers.
pub mod brain;
/// Profile, model catalog and whoami.
pub mod catalog;
/// The device-authenticated consumer API.
pub mod consumer;
/// The public discovery document.
pub mod discovery;
/// The Drive: user routes and the sync feed.
pub mod drive;
/// Error bodies.
pub mod envelope;
/// Header names the harness sends or reads.
pub mod headers;
/// The heartbeat.
pub mod heartbeat;
/// Governance hooks.
pub mod hooks;
/// The signed manifest.
pub mod manifest;
/// The reader's notification feed.
pub mod notifications;
/// Route paths.
pub mod paths;
