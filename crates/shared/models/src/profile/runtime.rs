//! Runtime configuration and enums.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub environment: Environment,

    #[serde(default)]
    pub log_level: LogLevel,

    #[serde(default)]
    pub output_format: OutputFormat,

    #[serde(default)]
    pub no_color: bool,

    #[serde(default)]
    pub non_interactive: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            environment: Environment::Development,
            log_level: LogLevel::Normal,
            output_format: OutputFormat::Text,
            no_color: false,
            non_interactive: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Development,
    Test,
    Staging,
    Production,
}

impl Environment {
    pub const fn is_development(&self) -> bool {
        matches!(self, Self::Development)
    }

    pub const fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    pub const fn is_test(&self) -> bool {
        matches!(self, Self::Test)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Test => write!(f, "test"),
            Self::Staging => write!(f, "staging"),
            Self::Production => write!(f, "production"),
        }
    }
}

impl std::str::FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "staging" => Ok(Self::Staging),
            "production" => Ok(Self::Production),
            _ => Err(format!(
                "Invalid environment '{}'. Must be one of: development, test, staging, production",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Quiet,
    #[default]
    Normal,
    Verbose,
    Debug,
}

impl LogLevel {
    pub const fn as_tracing_filter(&self) -> &'static str {
        match self {
            Self::Quiet => "error",
            Self::Normal => "info",
            Self::Verbose => "debug",
            Self::Debug => "trace",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quiet => write!(f, "quiet"),
            Self::Normal => write!(f, "normal"),
            Self::Verbose => write!(f, "verbose"),
            Self::Debug => write!(f, "debug"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quiet" => Ok(Self::Quiet),
            "normal" => Ok(Self::Normal),
            "verbose" => Ok(Self::Verbose),
            "debug" => Ok(Self::Debug),
            _ => Err(format!(
                "Invalid log level '{}'. Must be one of: quiet, normal, verbose, debug",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Yaml,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
            Self::Yaml => write!(f, "yaml"),
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            _ => Err(format!(
                "Invalid output format '{}'. Must be one of: text, json, yaml",
                s
            )),
        }
    }
}
