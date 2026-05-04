//! Shared value types used by [`super::ConfigManager`],
//! [`super::ConfigValidator`], and [`super::ConfigWriter`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, ConfigResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployEnvironment {
    Local,
    DockerDev,
    Production,
}

impl DeployEnvironment {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::DockerDev => "docker-dev",
            Self::Production => "production",
        }
    }

    pub fn parse(s: &str) -> ConfigResult<Self> {
        match s {
            "local" => Ok(Self::Local),
            "docker" | "docker-dev" => Ok(Self::DockerDev),
            "production" | "prod" => Ok(Self::Production),
            other => Err(ConfigError::other(format!("Invalid environment: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    #[serde(flatten)]
    pub vars: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    pub environment: DeployEnvironment,
    pub variables: HashMap<String, String>,
}
