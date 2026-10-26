//! Shared profile load/save for the `admin config` setter sub-trees.
//!
//! Every setter follows the same shape: load the layered profile (overlay over
//! `profiles/base.yaml`), mutate a typed field, write it back. `save_profile`
//! re-derives the URL fields, revalidates, and writes only the changed keys
//! into the overlay — never the base — so a config edit can never persist a
//! profile the loader would reject at boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_manifest::Profile;

use crate::context::CommandContext;

pub(super) fn profile_path(ctx: &CommandContext) -> Result<&str> {
    let path = ctx.loaded_profile()?.path();
    path.to_str()
        .with_context(|| format!("Profile path is not valid UTF-8: {}", path.display()))
}

pub(super) fn load_profile(path: &str) -> Result<Profile> {
    Profile::load_from_path(Path::new(path))
        .with_context(|| format!("Failed to load profile: {path}"))
}

pub(super) fn save_profile(profile: &Profile, path: &str) -> Result<()> {
    let mut profile = profile.clone();
    profile
        .derive_fields()
        .context("profile is invalid after edit; refusing to write")?;
    profile
        .validate()
        .context("profile is invalid after edit; refusing to write")?;
    profile
        .save_overlay(Path::new(path), None)
        .with_context(|| format!("Failed to write profile overlay: {path}"))
}

pub(super) fn profile_dir(path: &str) -> &Path {
    Path::new(path).parent().unwrap_or_else(|| Path::new("."))
}
