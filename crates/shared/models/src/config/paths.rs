#[derive(Debug, Clone)]
pub struct PathNotConfiguredError {
    pub path_name: String,
    pub profile_path: Option<String>,
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

impl std::fmt::Display for PathNotConfiguredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Profile Error: Required path not configured\n")?;
        writeln!(f, "  Field: paths.{}", self.path_name)?;
        if let Some(ref profile) = self.profile_path {
            writeln!(f, "  Profile: {}", profile)?;
        }
        writeln!(f, "\n  To fix:")?;
        writeln!(
            f,
            "  - Run 'systemprompt cloud config' to regenerate profile"
        )?;
        write!(
            f,
            "  - Or manually add paths.{} to your profile",
            self.path_name
        )
    }
}

impl std::error::Error for PathNotConfiguredError {}
