//! Reads, validates, and writes profile YAML files (with embedded
//! gateway / cloud catalogues).
//!
//! [`ProfileLoader`] is a thin shim over
//! [`systemprompt_config::load_profile_with_catalog`] that adds:
//!
//! - on-disk path conventions (`profiles/<name>.secrets.profile.yaml`),
//! - serialization with a leading "do not commit secrets" header, and
//! - directory enumeration for the `systemprompt cloud` CLI commands.

use std::path::Path;
use systemprompt_config::load_profile_with_catalog;
use systemprompt_models::Profile;

use crate::error::{ProfileLoadError, ProfileLoadResult};

#[derive(Debug, Clone, Copy)]
pub struct ProfileLoader;

impl ProfileLoader {
    pub fn load_from_path(profile_path: &Path) -> ProfileLoadResult<Profile> {
        load_profile_with_catalog(profile_path).map_err(ProfileLoadError::from)
    }

    pub fn load(services_path: &Path, profile_name: &str) -> ProfileLoadResult<Profile> {
        let profile_path = services_path
            .join("profiles")
            .join(format!("{profile_name}.secrets.profile.yaml"));

        Self::load_from_path(&profile_path)
    }

    pub fn load_from_path_and_validate(profile_path: &Path) -> ProfileLoadResult<Profile> {
        let profile = Self::load_from_path(profile_path)?;
        profile.validate().map_err(ProfileLoadError::from)?;
        Ok(profile)
    }

    pub fn load_and_validate(
        services_path: &Path,
        profile_name: &str,
    ) -> ProfileLoadResult<Profile> {
        let profile = Self::load(services_path, profile_name)?;
        profile.validate().map_err(ProfileLoadError::from)?;
        Ok(profile)
    }

    pub fn save(profile: &Profile, services_path: &Path) -> ProfileLoadResult<()> {
        let profiles_dir = services_path.join("profiles");
        std::fs::create_dir_all(&profiles_dir).map_err(|e| ProfileLoadError::Io {
            path: profiles_dir.clone(),
            source: e,
        })?;

        let profile_path = profiles_dir.join(format!("{}.secrets.profile.yaml", profile.name));
        let content = profile.to_yaml().map_err(ProfileLoadError::from)?;

        let content_with_header = format!(
            "# systemprompt.io Profile: {}\n# \n# WARNING: This file contains secrets.\n# DO NOT \
             commit to version control.\n\n{content}",
            profile.display_name
        );

        std::fs::write(&profile_path, content_with_header).map_err(|e| ProfileLoadError::Io {
            path: profile_path,
            source: e,
        })
    }

    #[must_use]
    pub fn list_available(services_path: &Path) -> Vec<String> {
        let profiles_dir = services_path.join("profiles");

        if !profiles_dir.exists() {
            return Vec::new();
        }

        match std::fs::read_dir(&profiles_dir) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.strip_suffix(".secrets.profile.yaml")
                        .map(str::to_owned)
                })
                .collect(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = %profiles_dir.display(),
                    "Failed to read profiles directory"
                );
                Vec::new()
            },
        }
    }
}
