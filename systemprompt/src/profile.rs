//! Profile types — the on-disk profile schema.
//!
//! `Profile`, `CloudConfig`, `ProfileStyle` and `CloudValidationMode`, plus the
//! `ProfileBootstrap` loader when the `config` feature is enabled and the
//! `ServicesState` (the services config, provider catalog and gateway routes) when `loader`
//! is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "config")]
pub use systemprompt_config::{ProfileBootstrap, ProfileBootstrapError};
#[cfg(feature = "loader")]
pub use systemprompt_loader::ServicesState;

pub use systemprompt_manifest::profile::{CloudConfig, CloudValidationMode, Profile, ProfileStyle};
