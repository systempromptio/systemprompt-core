pub mod admin_user;
pub mod content;
mod interactive;
mod prompt;
pub mod skills;

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand, ValueEnum};
use systemprompt_cloud::{get_cloud_paths, CloudPath, TenantStore};
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_sync::{SyncConfig, SyncDirection, SyncOperationResult, SyncService};

use crate::cli_settings::CliConfig;
use crate::cloud::tenant::get_credentials;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliLocalSyncDirection {
    ToDb,
    ToDisk,
}

#[derive(Debug, Subcommand)]
pub enum SyncCommands {
    Push(SyncArgs),

    Pull(SyncArgs),

    #[command(subcommand)]
    Local(LocalSyncCommands),
}

#[derive(Debug, Subcommand)]
pub enum LocalSyncCommands {
    Content(ContentSyncArgs),

    Skills(SkillsSyncArgs),
}

#[derive(Debug, Clone, Copy, Args)]
pub struct SyncArgs {
    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub force: bool,

    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Args)]
pub struct ContentSyncArgs {
    #[arg(long, value_enum)]
    pub direction: Option<CliLocalSyncDirection>,

    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long)]
    pub source: Option<String>,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub delete_orphans: bool,
}

#[derive(Debug, Args)]
pub struct SkillsSyncArgs {
    #[arg(long, value_enum)]
    pub direction: Option<CliLocalSyncDirection>,

    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long)]
    pub skill: Option<String>,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub delete_orphans: bool,
}

pub async fn execute(cmd: Option<SyncCommands>, config: &CliConfig) -> Result<()> {
    match cmd {
        Some(SyncCommands::Push(args)) => execute_cloud_sync(SyncDirection::Push, args).await,
        Some(SyncCommands::Pull(args)) => execute_cloud_sync(SyncDirection::Pull, args).await,
        Some(SyncCommands::Local(cmd)) => execute_local_sync(cmd).await,
        None => {
            if !config.is_interactive() {
                return Err(anyhow!(
                    "Sync subcommand required in non-interactive mode. Use push, pull, or local."
                ));
            }
            interactive::execute().await
        },
    }
}

async fn execute_local_sync(cmd: LocalSyncCommands) -> Result<()> {
    match cmd {
        LocalSyncCommands::Content(args) => content::execute(args).await,
        LocalSyncCommands::Skills(args) => skills::execute(args).await,
    }
}

async fn execute_cloud_sync(direction: SyncDirection, args: SyncArgs) -> Result<()> {
    let creds = get_credentials()?;

    let profile = ProfileBootstrap::get()
        .map_err(|_| anyhow!("Profile required for sync. Set SYSTEMPROMPT_PROFILE"))?;

    let tenant_id = profile
        .cloud
        .as_ref()
        .and_then(|c| c.tenant_id.as_ref())
        .ok_or_else(|| anyhow!("No tenant configured. Run 'systemprompt cloud profile create'"))?;

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });
    let tenant = store.find_tenant(tenant_id);

    if let Some(t) = tenant {
        if t.is_local() {
            return Err(anyhow!(
                "Cannot sync local tenant '{}' to cloud. Local tenants are for development \
                 only.\nCreate a cloud tenant with 'systemprompt cloud tenant create' or select \
                 an existing cloud tenant with 'systemprompt cloud profile create'.",
                tenant_id
            ));
        }
    }

    let (hostname, sync_token) =
        tenant.map_or((None, None), |t| (t.hostname.clone(), t.sync_token.clone()));

    let services_path = profile.paths.services.clone();

    let config = SyncConfig {
        direction,
        dry_run: args.dry_run,
        verbose: args.verbose,
        tenant_id: tenant_id.clone(),
        api_url: creds.api_url.clone(),
        api_token: creds.api_token.clone(),
        services_path,
        hostname,
        sync_token,
    };

    print_header(&direction, args.dry_run);

    let service = SyncService::new(config);
    let mut results = Vec::new();

    let spinner = CliService::spinner("Syncing files...");
    let files_result = service.sync_files().await?;
    spinner.finish_and_clear();
    results.push(files_result);

    print_results(&results);

    Ok(())
}

fn print_header(direction: &SyncDirection, dry_run: bool) {
    CliService::section("Cloud Sync");
    let dir = match direction {
        SyncDirection::Push => "Local -> Cloud",
        SyncDirection::Pull => "Cloud -> Local",
    };
    CliService::key_value("Direction", dir);
    if dry_run {
        CliService::warning("DRY RUN - no changes will be made");
    }
}

fn print_results(results: &[SyncOperationResult]) {
    for result in results {
        if result.success {
            CliService::success(&format!(
                "{} - Synced {} items",
                result.operation, result.items_synced
            ));
        } else {
            CliService::error(&format!(
                "{} - Failed with {} errors",
                result.operation,
                result.errors.len()
            ));
            for err in &result.errors {
                CliService::error(&format!("  - {}", err));
            }
        }
    }
}
