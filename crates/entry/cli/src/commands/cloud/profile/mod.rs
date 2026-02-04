mod api_keys;
mod builders;
mod create;
mod create_setup;
mod create_tenant;
mod delete;
mod edit;
mod edit_secrets;
mod edit_settings;
mod list;
mod show;
mod show_display;
mod show_types;
pub mod templates;

pub use api_keys::collect_api_keys;
pub use create::create_profile_for_tenant;
pub use create_setup::{get_cloud_user, handle_local_tenant_setup};

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

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
        long,
        env = "SYSTEMPROMPT_TENANT_ID",
        help = "Tenant ID (required in non-interactive mode)"
    )]
    pub tenant_id: Option<String>,

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

pub async fn execute(cmd: Option<ProfileCommands>, config: &CliConfig) -> Result<()> {
    if let Some(cmd) = cmd {
        execute_command(cmd, config).await.map(drop)
    } else {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "Profile subcommand required in non-interactive mode"
            ));
        }
        while let Some(cmd) = select_operation()? {
            if execute_command(cmd, config).await? {
                break;
            }
        }
        Ok(())
    }
}

async fn execute_command(cmd: ProfileCommands, config: &CliConfig) -> Result<bool> {
    match cmd {
        ProfileCommands::Create(args) => create::execute(&args, config).await.map(|()| true),
        ProfileCommands::List => {
            let result = list::execute(config)?;
            render_result(&result);
            Ok(false)
        },
        ProfileCommands::Show {
            name,
            filter,
            json,
            yaml,
        } => show::execute(name.as_deref(), filter, json, yaml, config).map(|()| false),
        ProfileCommands::Delete(args) => {
            let result = delete::execute(&args, config)?;
            render_result(&result);
            Ok(false)
        },
        ProfileCommands::Edit(args) => edit::execute(&args, config).await.map(|()| false),
    }
}

fn select_operation() -> Result<Option<ProfileCommands>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();
    let has_profiles = profiles_dir.exists()
        && std::fs::read_dir(&profiles_dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
            })
            .unwrap_or(false);

    let edit_label = if has_profiles {
        "Edit".to_string()
    } else {
        "Edit (unavailable - no profiles)".to_string()
    };
    let delete_label = if has_profiles {
        "Delete".to_string()
    } else {
        "Delete (unavailable - no profiles)".to_string()
    };

    let operations = vec![
        "List".to_string(),
        edit_label,
        delete_label,
        "Done".to_string(),
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile operation")
        .items(&operations)
        .default(0)
        .interact()?;

    let cmd = match selection {
        0 => Some(ProfileCommands::List),
        1 | 2 if !has_profiles => {
            CliService::warning("No profiles found");
            CliService::info(
                "Run 'systemprompt cloud tenant create' (or 'just tenant') to create a tenant \
                 with a profile.",
            );
            return Ok(Some(ProfileCommands::List));
        },
        1 => Some(ProfileCommands::Edit(EditArgs {
            name: None,
            set_anthropic_key: None,
            set_openai_key: None,
            set_gemini_key: None,
            set_github_token: None,
            set_database_url: None,
            set_external_url: None,
            set_host: None,
            set_port: None,
        })),
        2 => select_profile("Select profile to delete")?
            .map(|name| ProfileCommands::Delete(DeleteArgs { name, yes: false })),
        3 => None,
        _ => unreachable!(),
    };

    Ok(cmd)
}

fn select_profile(prompt: &str) -> Result<Option<String>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        CliService::warning("No profiles directory found.");
        return Ok(None);
    }

    let profiles: Vec<String> = std::fs::read_dir(&profiles_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    if profiles.is_empty() {
        CliService::warning("No profiles found.");
        return Ok(None);
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&profiles)
        .default(0)
        .interact()?;

    Ok(Some(profiles[selection].clone()))
}
