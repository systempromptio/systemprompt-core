//! Reads, validates, writes and enumerates layered profiles.
//!
//! [`ProfileLoader`] is a thin shim over the manifest's layered loader: a
//! profile is `<profiles_dir>/<env>/profile.yaml` over the optional shared
//! `<profiles_dir>/base.yaml`. Writes go to the overlay only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use systemprompt_identifiers::ProfileName;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::OVERLAY_PROFILE_FILE;

use crate::error::{ProfileLoadError, ProfileLoadResult};

#[derive(Debug, Clone, Copy)]
pub struct ProfileLoader;

impl ProfileLoader {
    pub fn load_from_path(profile_path: &Path) -> ProfileLoadResult<Profile> {
        Profile::load_from_path(profile_path).map_err(ProfileLoadError::from)
    }

    pub fn load(profiles_dir: &Path, profile_name: &ProfileName) -> ProfileLoadResult<Profile> {
        Profile::load_layered(profiles_dir, profile_name.as_str()).map_err(ProfileLoadError::from)
    }

    pub fn load_from_path_and_validate(profile_path: &Path) -> ProfileLoadResult<Profile> {
        let profile = Self::load_from_path(profile_path)?;
        profile.validate().map_err(ProfileLoadError::from)?;
        Ok(profile)
    }

    pub fn load_and_validate(
        profiles_dir: &Path,
        profile_name: &ProfileName,
    ) -> ProfileLoadResult<Profile> {
        let profile = Self::load(profiles_dir, profile_name)?;
        profile.validate().map_err(ProfileLoadError::from)?;
        Ok(profile)
    }

    pub fn save(profile: &Profile, profiles_dir: &Path) -> ProfileLoadResult<()> {
        let overlay_path = profiles_dir.join(&profile.name).join(OVERLAY_PROFILE_FILE);
        let header = format!(
            "# systemprompt.io Profile: {}\n# Overlay over ../base.yaml: holds only what this \
             environment changes.",
            profile.display_name
        );
        profile
            .save_overlay(&overlay_path, Some(&header))
            .map_err(ProfileLoadError::from)
    }

    #[must_use]
    pub fn list_available(profiles_dir: &Path) -> Vec<String> {
        let entries = match std::fs::read_dir(profiles_dir) {
            Ok(entries) => entries,
            Err(e) => {
                if profiles_dir.exists() {
                    tracing::warn!(
                        error = %e,
                        path = %profiles_dir.display(),
                        "Failed to read profiles directory"
                    );
                }
                return Vec::new();
            },
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().join(OVERLAY_PROFILE_FILE).is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }
}
