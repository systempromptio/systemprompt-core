mod create;
mod crud;
mod docker;
mod rotate;
mod select;
mod validation;

pub use create::{
    create_cloud_tenant, create_external_tenant, create_local_tenant, swap_to_external_host,
};
pub use crud::{cancel_subscription, delete_tenant, edit_tenant, list_tenants, show_tenant};
pub use docker::wait_for_postgres_healthy;
pub use rotate::{rotate_credentials, rotate_sync_token};
pub use select::{get_credentials, resolve_tenant_id};
pub use validation::{check_build_ready, find_services_config};

use anyhow::Result;
use clap::{Args, Subcommand};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{get_cloud_paths, CloudPath, TenantStore};
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;

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

    #[command(about = "Rotate sync token")]
    RotateSyncToken(TenantRotateArgs),

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

pub async fn execute(cmd: Option<TenantCommands>, config: &CliConfig) -> Result<()> {
    if let Some(cmd) = cmd {
        execute_command(cmd, config).await.map(drop)
    } else {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "Tenant subcommand required in non-interactive mode"
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

async fn execute_command(cmd: TenantCommands, config: &CliConfig) -> Result<bool> {
    match cmd {
        TenantCommands::Create { region } => tenant_create(&region, config).await.map(|()| true),
        TenantCommands::List => list_tenants(config).await.map(|()| false),
        TenantCommands::Show { id } => show_tenant(id, config).await.map(|()| false),
        TenantCommands::Delete(args) => delete_tenant(args, config).await.map(|()| false),
        TenantCommands::Edit { id } => edit_tenant(id, config).await.map(|()| false),
        TenantCommands::RotateCredentials(args) => {
            rotate_credentials(args.id, args.yes || !config.is_interactive())
                .await
                .map(|()| false)
        },
        TenantCommands::RotateSyncToken(args) => {
            rotate_sync_token(args.id, args.yes || !config.is_interactive())
                .await
                .map(|()| false)
        },
        TenantCommands::Cancel(args) => cancel_subscription(args, config).await.map(|()| false),
    }
}

fn select_operation() -> Result<Option<TenantCommands>> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });
    let has_tenants = !store.tenants.is_empty();

    let edit_label = if has_tenants {
        "Edit".to_string()
    } else {
        "Edit (unavailable - no tenants configured)".to_string()
    };
    let delete_label = if has_tenants {
        "Delete".to_string()
    } else {
        "Delete (unavailable - no tenants configured)".to_string()
    };

    let operations = vec![
        "Create".to_string(),
        "List".to_string(),
        edit_label,
        delete_label,
        "Done".to_string(),
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Tenant operation")
        .items(&operations)
        .default(0)
        .interact()?;

    let cmd = match selection {
        0 => Some(TenantCommands::Create {
            region: "iad".to_string(),
        }),
        1 => Some(TenantCommands::List),
        2 | 3 if !has_tenants => {
            CliService::warning("No tenants configured");
            CliService::info("Run 'systemprompt cloud tenant create' (or 'just tenant') to create one.");
            return Ok(Some(TenantCommands::List));
        },
        2 => Some(TenantCommands::Edit { id: None }),
        3 => Some(TenantCommands::Delete(TenantDeleteArgs {
            id: None,
            yes: false,
        })),
        4 => None,
        _ => unreachable!(),
    };

    Ok(cmd)
}

async fn tenant_create(default_region: &str, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Tenant creation requires interactive mode.\nUse specific tenant type commands in \
             non-interactive mode (not yet implemented)."
        ));
    }

    CliService::section("Create Tenant");

    let creds = get_credentials()?;

    let build_result = check_build_ready();
    let cloud_option = match &build_result {
        Ok(()) => "Cloud (requires subscription at systemprompt.io)".to_string(),
        Err(e) => {
            tracing::debug!(error = %e, "Build requirements check failed");
            "Cloud (unavailable - build requirements not met)".to_string()
        },
    };

    let options = vec![
        "Local (creates PostgreSQL container automatically)".to_string(),
        cloud_option,
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Tenant type")
        .items(&options)
        .default(0)
        .interact()?;

    let tenant = match selection {
        0 => {
            let db_options = vec![
                "Docker (creates PostgreSQL container automatically)",
                "External PostgreSQL (use your own database)",
            ];

            let db_selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Database source")
                .items(&db_options)
                .default(0)
                .interact()?;

            match db_selection {
                0 => create_local_tenant().await?,
                _ => create_external_tenant().await?,
            }
        },
        _ if build_result.is_err() => {
            CliService::warning("Cloud tenant requires a built project");
            if let Err(err) = build_result {
                CliService::error(&err);
            }
            return Ok(());
        },
        _ => create_cloud_tenant(&creds, default_region).await?,
    };

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    if let Some(existing) = store.tenants.iter_mut().find(|t| t.id == tenant.id) {
        *existing = tenant.clone();
    } else {
        store.tenants.push(tenant.clone());
    }
    store.save_to_path(&tenants_path)?;

    CliService::success("Tenant created");
    CliService::key_value("ID", &tenant.id);
    CliService::key_value("Name", &tenant.name);
    CliService::key_value("Type", &format!("{:?}", tenant.tenant_type));

    if let Some(ref url) = tenant.database_url {
        if !url.is_empty() {
            CliService::key_value("Database URL", url);
        }
    }

    Ok(())
}
