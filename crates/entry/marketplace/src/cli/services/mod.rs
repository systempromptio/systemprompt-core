//! `core services validate`: load a services tree the way an instance
//! would, optionally composed over a base bundle and compared against a
//! previous one.
//!
//! The `marketplace-http` member contributes the leaf under the kernel's
//! `core services` group, because the catalogue load it ends with is the
//! marketplace's. It reads and writes only the paths it is given and a
//! scratch directory: no profile, secrets or database, and no grant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod staging;
pub mod validate;
pub mod versions;

pub use validate::ValidateArgs;
