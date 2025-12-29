use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployEnvironment {
    Local,
    DockerDev,
    Production,
}

impl DeployEnvironment {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::DockerDev => "docker-dev",
            Self::Production => "production",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "local" => Ok(Self::Local),
            "docker" | "docker-dev" => Ok(Self::DockerDev),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(anyhow!("Invalid environment: {}", s)),
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
