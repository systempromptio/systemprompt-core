//! Shared value types used by [`super::ConfigManager`],
//! [`super::ConfigValidator`], and [`super::ConfigWriter`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, ConfigResult};

/// Deployment environments understood by the config-generation
/// pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployEnvironment {
    /// Local developer machine (`infrastructure/environments/local`).
    Local,
    /// Docker compose dev stack (`environments/docker-dev`).
    DockerDev,
    /// Production target (`environments/production`).
    Production,
}

impl DeployEnvironment {
    /// Lowercase string used as the directory name on disk.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::DockerDev => "docker-dev",
            Self::Production => "production",
        }
    }

    /// Parse a [`DeployEnvironment`] from CLI input.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Other`] when `s` does not match any
    /// known environment alias.
    pub fn parse(s: &str) -> ConfigResult<Self> {
        match s {
            "local" => Ok(Self::Local),
            "docker" | "docker-dev" => Ok(Self::DockerDev),
            "production" | "prod" => Ok(Self::Production),
            other => Err(ConfigError::other(format!("Invalid environment: {other}"))),
        }
    }
}

/// Free-form deployment config used as an input to higher-level
/// orchestration commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    /// Flattened key/value map matching `infrastructure/environments`.
    #[serde(flatten)]
    pub vars: HashMap<String, serde_yaml::Value>,
}

/// Fully-resolved environment configuration ready for serialization.
#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    /// Environment this config was generated for.
    pub environment: DeployEnvironment,
    /// `KEY -> VALUE` pairs after merge + variable substitution.
    pub variables: HashMap<String, String>,
}
