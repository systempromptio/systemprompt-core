//! `cloud tenant` subcommand: manage local and cloud tenants.
//!
//! Exposes [`TenantCommands`] (create, list, show, delete, edit, rotate
//! credentials, cancel) with an interactive operation menu when no subcommand
//! is supplied. Persists tenant records through the cloud `TenantStore`.

mod cancel;
mod create;
mod create_flow;
pub(super) mod delete;
pub mod docker;
mod edit;
mod list;
mod rotate;
pub mod select;
mod show;
mod validation;

pub use cancel::cancel_subscription;
pub use create::{
    create_cloud_tenant, create_external_tenant, create_local_tenant, handle_orphaned_volume,
    resolve_container_state, swap_to_external_host,
};
pub use delete::delete_tenant;
pub(in crate::commands::cloud) use docker::wait_for_postgres_healthy;
pub use edit::edit_tenant;
pub use list::list_tenants;
pub use rotate::rotate_credentials;
pub use select::{get_credentials, resolve_tenant_id, select_tenant};
pub use show::show_tenant;
pub use validation::{check_build_ready, validate_ai_config};

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use systemprompt_logging::CliService;

use crate::context::CommandContext;
use crate::interactive::Prompter;
use crate::shared::render_result;
use create_flow::tenant_create;

#[derive(Debug, Subcommand)]
pub enum TenantCommands {
    #[command(about = "Create a new tenant (local or cloud)")]
    Create {
        #[arg(long, default_value = "iad")]
        region: String,
    },

    #[command(
        about = "List all tenants",
        after_help = "EXAMPLES:\n  systemprompt cloud tenant list\n  systemprompt cloud tenant \
                      list --json"
    )]
    List,

    #[command(about = "Show tenant details")]
    Show { id: Option<String> },

    #[command(about = "Delete a tenant")]
    Delete(TenantDeleteArgs),

    #[command(about = "Edit tenant configuration")]
    Edit { id: Option<String> },

    #[command(about = "Rotate database credentials")]
    RotateCredentials(TenantRotateArgs),

    #[command(about = "Cancel subscription and destroy tenant (IRREVERSIBLE)")]
    Cancel(TenantCancelArgs),
}

#[derive(Debug, Args)]
pub struct TenantRotateArgs {
    pub id: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct TenantDeleteArgs {
    pub id: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct TenantCancelArgs {
    pub id: Option<String>,
}

pub async fn execute(cmd: Option<TenantCommands>, ctx: &CommandContext) -> Result<()> {
    if let Some(cmd) = cmd {
        execute_command(cmd, ctx).await.map(drop)
    } else {
        if !ctx.cli.is_interactive() {
            return Err(anyhow::anyhow!(
                "Tenant subcommand required in non-interactive mode"
            ));
        }
        while let Some(cmd) = select_operation(ctx.prompter())? {
            if execute_command(cmd, ctx).await? {
                break;
            }
        }
        Ok(())
    }
}

async fn execute_command(cmd: TenantCommands, ctx: &CommandContext) -> Result<bool> {
    match cmd {
        TenantCommands::Create { region } => tenant_create(&region, ctx.prompter(), &ctx.cli)
            .await
            .map(|()| true),
        TenantCommands::List => {
            let result = list_tenants(ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        TenantCommands::Show { id } => {
            let result = show_tenant(ctx.prompter(), id.as_ref(), &ctx.cli)?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        TenantCommands::Delete(args) => {
            let result = delete_tenant(args, ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        TenantCommands::Edit { id } => {
            let result = edit_tenant(id, ctx.prompter(), &ctx.cli)?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        TenantCommands::RotateCredentials(args) => {
            let result = rotate_credentials(
                args.id,
                args.yes || !ctx.cli.is_interactive(),
                ctx.prompter(),
                &ctx.cli,
            )
            .await?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        TenantCommands::Cancel(args) => {
            let result = cancel_subscription(args, ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
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
        0 => Some(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        1 => Some(TenantCommands::List),
        2 | 3 if !has_tenants => {
            CliService::warning("No tenants configured");
            CliService::info(
                "Run 'systemprompt cloud tenant create' (or 'just tenant') to create one.",
            );
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
