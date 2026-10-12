//! Profile types — the on-disk profile schema.
//!
//! `Profile`, `CloudConfig`, `ProfileStyle` and `CloudValidationMode`, plus the
//! `ProfileBootstrap` loader when the `config` feature is enabled and the
//! `ServicesBootstrap` cell (provider catalog, gateway routes) when `loader`
//! is.

#[cfg(feature = "config")]
pub use systemprompt_config::{ProfileBootstrap, ProfileBootstrapError};
#[cfg(feature = "loader")]
pub use systemprompt_loader::ServicesBootstrap;

pub use systemprompt_manifest::profile::{CloudConfig, CloudValidationMode, Profile, ProfileStyle};
