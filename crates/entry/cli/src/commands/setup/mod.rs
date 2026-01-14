mod docker;
mod postgres;
mod profile;
mod secrets;
mod types;
mod wizard;

use anyhow::Result;
use clap::Args;
use systemprompt_models::cli::CommandResult;

pub use types::*;

#[derive(Debug, Args)]
pub struct SetupArgs {
    #[arg(
        short,
        long,
        help = "Target environment name (e.g., dev, staging, prod)"
    )]
    pub environment: Option<String>,

    #[arg(
        long,
        help = "Use Docker for PostgreSQL (default: use existing installation)"
    )]
    pub docker: bool,

    #[arg(
        long,
        env = "SYSTEMPROMPT_DB_HOST",
        default_value = "localhost",
        help = "PostgreSQL host"
    )]
    pub db_host: String,

    #[arg(
        long,
        env = "SYSTEMPROMPT_DB_PORT",
        default_value = "5432",
        help = "PostgreSQL port"
    )]
    pub db_port: u16,

    #[arg(
        long,
        env = "SYSTEMPROMPT_DB_USER",
        help = "PostgreSQL user (default: systemprompt_`<env>`)"
    )]
    pub db_user: Option<String>,

    #[arg(
        long,
        env = "SYSTEMPROMPT_DB_PASSWORD",
        help = "PostgreSQL password (auto-generated if not provided)"
    )]
    pub db_password: Option<String>,

    #[arg(
        long,
        env = "SYSTEMPROMPT_DB_NAME",
        help = "PostgreSQL database name (default: systemprompt_`<env>`)"
    )]
    pub db_name: Option<String>,

    #[arg(long, env = "GEMINI_API_KEY", help = "Google AI (Gemini) API key")]
    pub gemini_key: Option<String>,

    #[arg(long, env = "ANTHROPIC_API_KEY", help = "Anthropic (Claude) API key")]
    pub anthropic_key: Option<String>,

    #[arg(long, env = "OPENAI_API_KEY", help = "OpenAI (GPT) API key")]
    pub openai_key: Option<String>,

    #[arg(long, env = "GITHUB_TOKEN", help = "GitHub token (optional)")]
    pub github_token: Option<String>,

    #[arg(long, help = "Run database migrations after setup")]
    pub migrate: bool,

    #[arg(
        long,
        conflicts_with = "migrate",
        help = "Skip migrations (non-interactive default)"
    )]
    pub no_migrate: bool,

    #[arg(long, help = "Preview setup without creating files or making changes")]
    pub dry_run: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
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

    pub const fn has_ai_provider(&self) -> bool {
        self.gemini_key.is_some() || self.anthropic_key.is_some() || self.openai_key.is_some()
    }
}

pub async fn execute(
    args: SetupArgs,
    config: &crate::CliConfig,
) -> Result<CommandResult<SetupOutput>> {
    wizard::execute(args, config).await
}
