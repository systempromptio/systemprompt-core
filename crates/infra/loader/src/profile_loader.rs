use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_models::Profile;

#[derive(Debug, Clone, Copy)]
pub struct ProfileLoader;

impl ProfileLoader {
    pub fn load_from_path(profile_path: &Path) -> Result<Profile> {
        let content = std::fs::read_to_string(profile_path)
            .with_context(|| format!("Failed to read profile: {}", profile_path.display()))?;

        Profile::parse(&content, profile_path)
    }

    pub fn load(services_path: &Path, profile_name: &str) -> Result<Profile> {
        let profile_path = services_path
            .join("profiles")
            .join(format!("{profile_name}.secrets.profile.yaml"));

        Self::load_from_path(&profile_path)
    }

    pub fn load_from_path_and_validate(profile_path: &Path) -> Result<Profile> {
        let profile = Self::load_from_path(profile_path)?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn load_and_validate(services_path: &Path, profile_name: &str) -> Result<Profile> {
        let profile = Self::load(services_path, profile_name)?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn save(profile: &Profile, services_path: &Path) -> Result<()> {
        let profiles_dir = services_path.join("profiles");
        std::fs::create_dir_all(&profiles_dir).context("Failed to create profiles directory")?;

        let profile_path = profiles_dir.join(format!("{}.secrets.profile.yaml", profile.name));
        let content = profile.to_yaml()?;

        let content_with_header = format!(
            "# systemprompt.io Profile: {}\n# \n# WARNING: This file contains secrets.\n# DO NOT \
             commit to version control.\n\n{content}",
            profile.display_name
        );

        std::fs::write(&profile_path, content_with_header)
            .with_context(|| format!("Failed to write profile: {}", profile_path.display()))
    }

    pub fn list_available(services_path: &Path) -> Vec<String> {
        let profiles_dir = services_path.join("profiles");

        if !profiles_dir.exists() {
            return Vec::new();
        }

        match std::fs::read_dir(&profiles_dir) {
            Ok(entries) => entries
                .filter_map(std::result::Result::ok)
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.strip_suffix(".secrets.profile.yaml")
                        .map(ToString::to_string)
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
