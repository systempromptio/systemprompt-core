//! Clap argument and command types for `cloud profile`.

use clap::{Args, Subcommand, ValueEnum};
use systemprompt_models::none_if_blank;

#[derive(Debug, Subcommand)]
pub enum ProfileCommands {
    #[command(about = "Create a new profile", hide = true)]
    Create(CreateArgs),

    #[command(about = "List all profiles")]
    List,

    #[command(
        about = "Show profile configuration",
        after_help = "EXAMPLES:\n  systemprompt cloud profile show\n  systemprompt cloud profile \
                      show --filter agents\n  systemprompt cloud profile show --json"
    )]
    Show {
        name: Option<String>,

        #[arg(short, long, value_enum, default_value = "all")]
        filter: ShowFilter,

        #[arg(long, help = "Output as JSON")]
        json: bool,

        #[arg(long, help = "Output as YAML")]
        yaml: bool,
    },

    #[command(about = "Delete a profile")]
    Delete(DeleteArgs),

    #[command(about = "Edit profile configuration")]
    Edit(EditArgs),
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub name: String,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    pub name: String,

    #[arg(
        long = "tenant-id",
        env = "SYSTEMPROMPT_TENANT_ID",
        help = "Tenant ID (required in non-interactive mode)"
    )]
    pub tenant: Option<String>,

    #[arg(long, value_enum, default_value = "local", help = "Tenant type")]
    pub tenant_type: TenantTypeArg,

    #[arg(long, env = "ANTHROPIC_API_KEY", help = "Anthropic (Claude) API key")]
    pub anthropic_key: Option<String>,

    #[arg(long, env = "OPENAI_API_KEY", help = "OpenAI (GPT) API key")]
    pub openai_key: Option<String>,

    #[arg(long, env = "GEMINI_API_KEY", help = "Google AI (Gemini) API key")]
    pub gemini_key: Option<String>,

    #[arg(long, env = "GITHUB_TOKEN", help = "GitHub token (optional)")]
    pub github_token: Option<String>,
}

impl CreateArgs {
    pub const fn has_api_key(&self) -> bool {
        self.anthropic_key.is_some() || self.openai_key.is_some() || self.gemini_key.is_some()
    }

    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.anthropic_key = none_if_blank(self.anthropic_key);
        self.openai_key = none_if_blank(self.openai_key);
        self.gemini_key = none_if_blank(self.gemini_key);
        self.github_token = none_if_blank(self.github_token);
        self
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TenantTypeArg {
    Local,
    Cloud,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    pub name: Option<String>,

    #[arg(long, help = "Set Anthropic API key")]
    pub set_anthropic_key: Option<String>,

    #[arg(long, help = "Set OpenAI API key")]
    pub set_openai_key: Option<String>,

    #[arg(long, help = "Set Gemini API key")]
    pub set_gemini_key: Option<String>,

    #[arg(long, help = "Set GitHub token")]
    pub set_github_token: Option<String>,

    #[arg(long, help = "Set database URL")]
    pub set_database_url: Option<String>,

    #[arg(long, help = "Set external URL (cloud profiles)")]
    pub set_external_url: Option<String>,

    #[arg(long, help = "Set server host")]
    pub set_host: Option<String>,

    #[arg(long, help = "Set server port")]
    pub set_port: Option<u16>,
}

impl EditArgs {
    pub const fn has_updates(&self) -> bool {
        self.set_anthropic_key.is_some()
            || self.set_openai_key.is_some()
            || self.set_gemini_key.is_some()
            || self.set_github_token.is_some()
            || self.set_database_url.is_some()
            || self.set_external_url.is_some()
            || self.set_host.is_some()
            || self.set_port.is_some()
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ShowFilter {
    All,
    Agents,
    Mcp,
    Skills,
    Ai,
    Web,
    Content,
    Env,
    Settings,
}
