//! Interactive and non-interactive setup wizard for a local environment.
//!
//! Drives `PostgreSQL` provisioning, secret collection, profile generation, and
//! optional migrations. [`SetupArgs`] captures the CLI flags and environment
//! overrides; [`execute`] dispatches to the wizard, which writes a profile and
//! secrets file under `.systemprompt/`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod catalog;
pub mod ddl;
pub mod docker;
pub mod docker_compose;
mod docker_database;
pub mod postgres;
pub mod postgres_access;
mod profile;
mod profile_sections;
pub mod provision;
pub mod secrets;
pub mod services_files;
mod types;
pub mod wizard;
mod wizard_dry_run;
#[doc(hidden)]
pub mod wizard_prompts;

use crate::shared::CommandOutput;
use anyhow::Result;
use clap::Args;
use systemprompt_manifest::none_if_blank;
use systemprompt_models::defaults::DEFAULT_POSTGRES_PORT;

pub use secrets::SecretsData;
pub use types::{DatabaseSetupInfo, SecretsConfiguredInfo, SetupOutput};

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent clap CLI flag, not a state machine"
)]
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

    #[arg(long, default_value = "localhost", help = "PostgreSQL host")]
    pub db_host: String,

    #[arg(long, default_value_t = DEFAULT_POSTGRES_PORT, help = "PostgreSQL port")]
    pub db_port: u16,

    #[arg(
        long,
        default_value = "0",
        help = "Shift every locally-bound MCP and agent port by this amount"
    )]
    pub port_offset: u16,

    #[arg(long, help = "PostgreSQL user (default: systemprompt_`<env>`)")]
    pub db_user: Option<String>,

    #[arg(
        long,
        help = "PostgreSQL password (auto-generated if not provided); visible in the process \
                list, prefer --db-password-stdin"
    )]
    pub db_password: Option<String>,

    #[arg(
        long,
        conflicts_with = "db_password",
        help = "Read the PostgreSQL password from the first line of stdin"
    )]
    pub db_password_stdin: bool,

    #[arg(
        long,
        help = "Change the password of an already existing role to the one this setup writes \
                (anything else using the role loses access; asks for confirmation)"
    )]
    pub reset_role_password: bool,

    #[arg(
        long,
        help = "PostgreSQL database name (default: systemprompt_`<env>`)"
    )]
    pub db_name: Option<String>,

    #[arg(
        long,
        env = "GEMINI_API_KEY",
        hide_env_values = true,
        help = "Google AI (Gemini) API key"
    )]
    pub gemini_key: Option<String>,

    #[arg(
        long,
        env = "ANTHROPIC_API_KEY",
        hide_env_values = true,
        help = "Anthropic (Claude) API key"
    )]
    pub anthropic_key: Option<String>,

    #[arg(
        long,
        env = "OPENAI_API_KEY",
        hide_env_values = true,
        help = "OpenAI (GPT) API key"
    )]
    pub openai_key: Option<String>,

    #[arg(
        long,
        env = "GITHUB_TOKEN",
        hide_env_values = true,
        help = "GitHub token (optional)"
    )]
    pub github_token: Option<String>,

    #[arg(
        long,
        help = "Provider to make the default (gemini | anthropic | openai); must have a key. \
                In interactive mode the selected provider is used instead."
    )]
    pub default_provider: Option<String>,

    #[arg(
        long,
        env = "SYSTEMPROMPT_ADMIN_EMAIL",
        hide_env_values = true,
        help = "Email identifying the platform admin on sign-in and consent screens; required non-interactively"
    )]
    pub admin_email: Option<String>,

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

    #[arg(
        long,
        help = "Overwrite existing profile/secrets files (default: preserve them)"
    )]
    pub force: bool,
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

    #[must_use]
    fn normalized(mut self) -> Self {
        self.gemini_key = none_if_blank(self.gemini_key);
        self.anthropic_key = none_if_blank(self.anthropic_key);
        self.openai_key = none_if_blank(self.openai_key);
        self.github_token = none_if_blank(self.github_token);
        self.default_provider = none_if_blank(self.default_provider);
        self.admin_email = none_if_blank(self.admin_email);
        self
    }
}

pub async fn execute(
    args: SetupArgs,
    ctx: &crate::context::CommandContext,
) -> Result<CommandOutput> {
    let mut args = args.normalized();
    if args.db_password_stdin {
        args.db_password = Some(read_password_line(std::io::stdin().lock())?);
    }
    wizard::execute(args, ctx.prompter(), &ctx.cli).await
}

pub fn read_password_line(mut reader: impl std::io::BufRead) -> Result<String> {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| anyhow::anyhow!("Failed to read the database password from stdin: {e}"))?;
    let password = line.trim_end_matches(['\r', '\n']).to_owned();
    if password.is_empty() {
        anyhow::bail!("--db-password-stdin read an empty password");
    }
    Ok(password)
}
