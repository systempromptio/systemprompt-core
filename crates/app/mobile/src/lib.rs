//! `systemprompt-mobile` — the binding layer for phones.
//!
//! The one surface the iOS and Android apps call, annotated for the binding
//! generator. A crate in the extension links this one, adds the fork's own
//! annotated members and builds the library each app loads; the Swift and
//! Kotlin that call it are generated from that library and never committed.
//!
//! Exported today: [`Instances`], which reads an instance's discovery
//! document, and [`diagnostics`]. Every failure crosses the boundary as a
//! [`MobileError`]. No token, credential or key type is exported, and no
//! exported call returns one.
//!
//! # Feature flags
//!
//! This crate has no feature flags.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod address;
mod diagnostics;
mod error;
mod instances;
mod tls;

pub use address::InstanceAddress;
pub use diagnostics::{Diagnostics, diagnostics};
pub use error::MobileError;
pub use instances::{AppRelease, InstanceDiscovery, Instances};

uniffi::setup_scaffolding!();
