mod docker;
mod postgres;
mod profile;
mod secrets;
mod wizard;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct SetupArgs {
    /// Target environment name (e.g., dev, staging, prod)
    #[arg(short, long)]
    pub environment: Option<String>,

    /// Use Docker for PostgreSQL (default: use existing installation)
    #[arg(long)]
    pub docker: bool,

    /// PostgreSQL host
    #[arg(long, default_value = "localhost")]
    pub db_host: String,

    /// PostgreSQL port
    #[arg(long, default_value = "5432")]
    pub db_port: u16,

    /// PostgreSQL user (default: systemprompt_<env>)
    #[arg(long)]
    pub db_user: Option<String>,

    /// PostgreSQL password (auto-generated if not provided)
    #[arg(long)]
    pub db_password: Option<String>,

    /// PostgreSQL database name (default: systemprompt_<env>)
    #[arg(long)]
    pub db_name: Option<String>,

    /// Google AI (Gemini) API key
    #[arg(long, env = "GEMINI_API_KEY")]
    pub gemini_key: Option<String>,

    /// Anthropic (Claude) API key
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    pub anthropic_key: Option<String>,

    /// OpenAI (GPT) API key
    #[arg(long, env = "OPENAI_API_KEY")]
    pub openai_key: Option<String>,

    /// GitHub token (optional)
    #[arg(long, env = "GITHUB_TOKEN")]
    pub github_token: Option<String>,

    /// Run database migrations after setup
    #[arg(long)]
    pub migrate: bool,

    /// Skip migrations (non-interactive default)
    #[arg(long, conflicts_with = "migrate")]
    pub no_migrate: bool,
}

impl SetupArgs {
    pub fn effective_db_user(&self, env_name: &str) -> String {
        self.db_user
            .clone()
            .unwrap_or_else(|| format!("systemprompt_{}", env_name))
    }

    pub fn effective_db_name(&self, env_name: &str) -> String {
        self.db_name
            .clone()
            .unwrap_or_else(|| format!("systemprompt_{}", env_name))
    }

    pub fn has_ai_provider(&self) -> bool {
        self.gemini_key.is_some() || self.anthropic_key.is_some() || self.openai_key.is_some()
    }
}

pub async fn execute(args: SetupArgs, config: &crate::CliConfig) -> Result<()> {
    wizard::execute(args, config).await
}
