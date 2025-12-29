//! Verbosity level configuration.

use super::Environment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbosityLevel {
    Quiet,
    Normal,
    Verbose,
    Debug,
}

impl VerbosityLevel {
    pub const fn from_environment(env: Environment) -> Self {
        match env {
            Environment::Development => Self::Verbose,
            Environment::Production => Self::Quiet,
            Environment::Test => Self::Normal,
        }
    }

    pub fn from_env_var() -> Option<Self> {
        if std::env::var("SYSTEMPROMPT_QUIET").ok().as_deref() == Some("1") {
            return Some(Self::Quiet);
        }

        if std::env::var("SYSTEMPROMPT_VERBOSE").ok().as_deref() == Some("1") {
            return Some(Self::Verbose);
        }

        if std::env::var("SYSTEMPROMPT_DEBUG").ok().as_deref() == Some("1") {
            return Some(Self::Debug);
        }

        if let Ok(level) = std::env::var("SYSTEMPROMPT_LOG_LEVEL") {
            return match level.to_lowercase().as_str() {
                "quiet" => Some(Self::Quiet),
                "normal" => Some(Self::Normal),
                "verbose" => Some(Self::Verbose),
                "debug" => Some(Self::Debug),
                _ => None,
            };
        }

        None
    }

    pub fn resolve() -> Self {
        if let Some(level) = Self::from_env_var() {
            return level;
        }

        let env = Environment::detect();
        Self::from_environment(env)
    }

    pub const fn is_quiet(&self) -> bool {
        matches!(self, Self::Quiet)
    }

    pub const fn is_verbose(&self) -> bool {
        matches!(self, Self::Verbose | Self::Debug)
    }

    pub const fn should_show_verbose(&self) -> bool {
        matches!(self, Self::Verbose | Self::Debug)
    }

    pub const fn should_log_to_db(&self) -> bool {
        !matches!(self, Self::Quiet)
    }
}
