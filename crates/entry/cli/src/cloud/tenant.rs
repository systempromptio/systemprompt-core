use anyhow::Result;
use clap::Subcommand;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{get_cloud_paths, CloudPath, TenantStore};
use systemprompt_core_logging::CliService;

use super::tenant_ops::{
    check_build_ready, create_cloud_tenant, create_local_tenant, delete_tenant, edit_tenant,
    get_credentials, list_tenants, show_tenant,
};

#[derive(Subcommand)]
pub enum TenantCommands {
    #[command(about = "Create a new tenant (local or cloud)")]
    Create {
        #[arg(long, default_value = "iad")]
        region: String,
    },

    #[command(about = "List all tenants")]
    List,

    #[command(about = "Show tenant details")]
    Show { id: Option<String> },

    #[command(about = "Delete a tenant")]
    Delete { id: Option<String> },

    #[command(about = "Edit tenant configuration")]
    Edit { id: Option<String> },
}

pub async fn execute(cmd: Option<TenantCommands>) -> Result<()> {
    match cmd {
        Some(cmd) => execute_command(cmd).await.map(|_| ()),
        None => {
            loop {
                match select_operation()? {
                    Some(cmd) => {
                        if execute_command(cmd).await? {
                            break;
                        }
                    },
                    None => break,
                }
            }
            Ok(())
        },
    }
}

async fn execute_command(cmd: TenantCommands) -> Result<bool> {
    match cmd {
        TenantCommands::Create { region } => create(&region).await.map(|_| true),
        TenantCommands::List => list_tenants().await.map(|_| false),
        TenantCommands::Show { id } => show_tenant(id).await.map(|_| false),
        TenantCommands::Delete { id } => delete_tenant(id).await.map(|_| false),
        TenantCommands::Edit { id } => edit_tenant(id).await.map(|_| false),
    }
}

fn select_operation() -> Result<Option<TenantCommands>> {
    let operations = ["Create", "List", "Edit", "Delete", "Done"];

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
        2 => Some(TenantCommands::Edit { id: None }),
        3 => Some(TenantCommands::Delete { id: None }),
        4 => None,
        _ => unreachable!(),
    };

    Ok(cmd)
}

async fn create(default_region: &str) -> Result<()> {
    CliService::section("Create Tenant");

    let creds = get_credentials()?;

    let build_result = check_build_ready();
    let cloud_option = match &build_result {
        Ok(()) => "Cloud (requires subscription at systemprompt.io)".to_string(),
        Err(_) => "Cloud (unavailable - build requirements not met)".to_string(),
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
        0 => create_local_tenant().await?,
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
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

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

    if tenant.has_database_url() {
        CliService::success("Database URL configured");
    }

    Ok(())
}
