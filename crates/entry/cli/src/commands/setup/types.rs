use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Output structure for the setup command
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetupOutput {
    /// Environment name (e.g., dev, staging, prod)
    pub environment: String,
    /// Path to the created profile file
    pub profile_path: String,
    /// Database configuration details
    pub database: DatabaseSetupInfo,
    /// Secrets configuration status
    pub secrets_configured: SecretsConfiguredInfo,
    /// Whether migrations were run
    pub migrations_run: bool,
    /// Human-readable status message
    pub message: String,
}

/// Database setup information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseSetupInfo {
    /// Database host
    pub host: String,
    /// Database port
    pub port: u16,
    /// Database name
    pub name: String,
    /// Database user
    pub user: String,
    /// Connection status (connected, unreachable, not_tested)
    pub connection_status: String,
    /// Whether Docker was used
    pub docker: bool,
}

/// Information about which secrets/API keys are configured
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecretsConfiguredInfo {
    /// Anthropic API key configured
    pub anthropic: bool,
    /// OpenAI API key configured
    pub openai: bool,
    /// Google AI (Gemini) API key configured
    pub gemini: bool,
    /// GitHub token configured
    pub github: bool,
}

impl SecretsConfiguredInfo {
    pub fn summary(&self) -> String {
        let mut keys = Vec::new();
        if self.anthropic {
            keys.push("Anthropic");
        }
        if self.openai {
            keys.push("OpenAI");
        }
        if self.gemini {
            keys.push("Gemini");
        }
        if self.github {
            keys.push("GitHub");
        }

        if keys.is_empty() {
            "None".to_string()
        } else {
            keys.join(", ")
        }
    }
}
