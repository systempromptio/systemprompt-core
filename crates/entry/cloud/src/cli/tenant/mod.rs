//! `cloud tenant` subcommand: manage local and cloud tenants.
//!
//! Exposes [`TenantCommands`] (create, add a self-managed record, list,
//! show, delete, edit, rotate credentials) with an interactive operation menu
//! when no subcommand is supplied. Persists tenant records through the cloud
//! `TenantStore`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod add;
pub mod create;
mod create_flow;
pub(super) mod delete;
pub mod docker;
mod edit;
mod list;
mod rotate;
pub mod select;
mod show;

pub use add::{TenantAddArgs, add_tenant};
pub use create::{create_external_tenant, create_local_tenant};
pub use delete::delete_tenant;
pub(in crate::cli) use docker::container::wait_for_postgres_healthy;
pub use edit::edit_tenant;
pub use list::list_tenants;
pub use rotate::rotate_credentials;
pub use select::{get_credentials, resolve_tenant_id, select_tenant};
pub use show::show_tenant;

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;

use create_flow::tenant_create;
use systemprompt_extension::cli::parsers::parse_tenant_id;
use systemprompt_extension::cli::{CommandHost, CommandOutput, Prompter};

#[derive(Debug, Subcommand)]
pub enum TenantCommands {
    #[command(about = "Create a new tenant")]
    Create,

    #[command(
        about = "Record a self-managed tenant (Kubernetes, GKE, local Docker)",
        after_help = "EXAMPLES:\n  systemprompt cloud tenant add --id acme-prod --name \"Acme \
                      Prod\" --hostname ai.acme.example --provider kubernetes"
    )]
    Add(TenantAddArgs),

    #[command(
        about = "List all tenants",
        after_help = "EXAMPLES:\n  systemprompt cloud tenant list\n  systemprompt cloud tenant \
                      list --json"
    )]
    List,

    #[command(about = "Show tenant details")]
    Show {
        #[arg(value_parser = parse_tenant_id)]
        id: Option<TenantId>,
    },

    #[command(about = "Delete a tenant")]
    Delete(TenantDeleteArgs),

    #[command(about = "Edit tenant configuration")]
    Edit {
        #[arg(value_parser = parse_tenant_id)]
        id: Option<TenantId>,
    },

    #[command(about = "Rotate database credentials")]
    RotateCredentials(TenantRotateArgs),
}

#[derive(Debug, Args)]
pub struct TenantRotateArgs {
    #[arg(value_parser = parse_tenant_id)]
    pub id: Option<TenantId>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct TenantDeleteArgs {
    #[arg(value_parser = parse_tenant_id)]
    pub id: Option<TenantId>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(cmd: Option<TenantCommands>, host: &dyn CommandHost) -> Result<CommandOutput> {
    if let Some(cmd) = cmd {
        return Ok(execute_command(cmd, host).await?.0);
    }
    if !host.output().is_interactive() {
        return Err(anyhow::anyhow!(
            "Tenant subcommand required in non-interactive mode"
        ));
    }
    while let Some(cmd) = select_operation(host.prompter())? {
        let (output, done) = execute_command(cmd, host).await?;
        host.render(&output);
        if done {
            break;
        }
    }
    Ok(CommandOutput::rendered())
}

async fn execute_command(
    cmd: TenantCommands,
    host: &dyn CommandHost,
) -> Result<(CommandOutput, bool)> {
    let (cli, prompter) = (host.output(), host.prompter());
    match cmd {
        TenantCommands::Create => {
            tenant_create(prompter, cli, host.children()).await?;
            Ok((CommandOutput::rendered(), true))
        },
        TenantCommands::Add(args) => Ok((add_tenant(args, cli)?, false)),
        TenantCommands::List => Ok((list_tenants(prompter, cli).await?, false)),
        TenantCommands::Show { id } => Ok((show_tenant(prompter, id.as_ref(), cli)?, false)),
        TenantCommands::Delete(args) => Ok((
            delete_tenant(args, prompter, cli, host.children()).await?,
            false,
        )),
        TenantCommands::Edit { id } => Ok((edit_tenant(id, prompter, cli)?, false)),
        TenantCommands::RotateCredentials(args) => Ok((
            rotate_credentials(args.id, args.yes || !cli.is_interactive(), prompter, cli).await?,
            false,
        )),
    }
}

fn select_operation(prompter: &dyn Prompter) -> Result<Option<TenantCommands>> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });
    let has_tenants = !store.tenants.is_empty();

    choose_tenant_operation(prompter, has_tenants)
}

pub fn choose_tenant_operation(
    prompter: &dyn Prompter,
    has_tenants: bool,
) -> Result<Option<TenantCommands>> {
    let edit_label = if has_tenants {
        "Edit".to_owned()
    } else {
        "Edit (unavailable - no tenants configured)".to_owned()
    };
    let delete_label = if has_tenants {
        "Delete".to_owned()
    } else {
        "Delete (unavailable - no tenants configured)".to_owned()
    };

    let operations = vec![
        "Create".to_owned(),
        "List".to_owned(),
        edit_label,
        delete_label,
        "Done".to_owned(),
    ];

    let selection = prompter.select("Tenant operation", &operations)?;

    let cmd = match selection {
        0 => Some(TenantCommands::Create),
        1 => Some(TenantCommands::List),
        2 | 3 if !has_tenants => {
            CliService::warning("No tenants configured");
            CliService::info("Run 'systemprompt cloud tenant create' to create one.");
            return Ok(Some(TenantCommands::List));
        },
        2 => Some(TenantCommands::Edit { id: None }),
        3 => Some(TenantCommands::Delete(TenantDeleteArgs {
            id: None,
            yes: false,
        })),
        4 => None,
        other => return Err(anyhow::anyhow!("unexpected menu selection: {other}")),
    };

    Ok(cmd)
}
