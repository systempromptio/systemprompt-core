//! Typed error for unconfigured path lookups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, thiserror::Error)]
#[error(
    "Profile Error: Required path not configured\n\n  Field: paths.{path_name}\n{}\n  \
     To fix:\n  - Run 'systemprompt cloud config' to regenerate profile\n  - Or manually add \
     paths.{path_name} to your profile",
    profile_line(.profile_path.as_deref())
)]
pub struct PathNotConfiguredError {
    pub path_name: String,
    pub profile_path: Option<String>,
}

fn profile_line(profile_path: Option<&str>) -> String {
    profile_path.map_or_else(String::new, |profile| format!("  Profile: {profile}\n"))
}

impl PathNotConfiguredError {
    pub fn new(path_name: impl Into<String>) -> Self {
        Self {
            path_name: path_name.into(),
            profile_path: None,
        }
    }

    pub fn with_profile_path(mut self, profile_path: impl Into<String>) -> Self {
        self.profile_path = Some(profile_path.into());
        self
    }
}
