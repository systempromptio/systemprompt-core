//! Path vocabulary shared by every layer: well-known directory and file names
//! and the [`PathResolution`] mode a profile selects.
//!
//! The filesystem projection of a profile (`AppPaths` and its parts) lives in
//! `systemprompt_config::paths`; this module holds only data.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod constants;

pub use constants::{cloud_container, dir_names, file_names};

/// How profile paths are resolved against the local filesystem. Derive the
/// right mode for a profile with [`crate::profile::Profile::path_resolution`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathResolution {
    Canonicalize,
    Lexical,
}
