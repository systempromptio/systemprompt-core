use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetupOutput {
    pub environment: String,
    pub profile_path: String,
    pub database: DatabaseSetupInfo,
    pub secrets_configured: SecretsConfiguredInfo,
    pub migrations_run: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseSetupInfo {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub connection_status: String,
    pub docker: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct SecretsConfiguredInfo {
    pub anthropic: bool,
    pub openai: bool,
    pub gemini: bool,
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
