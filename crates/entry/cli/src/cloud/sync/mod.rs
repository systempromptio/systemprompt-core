pub mod admin_user;
pub mod content;
mod prompt;
pub mod skills;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::SecretsBootstrap;
use systemprompt_sync::{SyncConfig, SyncDirection, SyncOperationResult, SyncService};

#[derive(Subcommand)]
pub enum SyncCommands {
    /// Push files and database to cloud
    Push(SyncArgs),

    /// Pull files and database from cloud
    Pull(SyncArgs),

    /// Sync between local disk and database
    #[command(subcommand)]
    Local(LocalSyncCommands),
}

#[derive(Subcommand)]
pub enum LocalSyncCommands {
    /// Sync content (blog, legal) between disk and local database
    Content(ContentSyncArgs),

    /// Sync skills between disk and local database
    Skills(SkillsSyncArgs),
}

#[derive(Args)]
pub struct SyncArgs {
    /// Preview changes without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation prompts
    #[arg(long)]
    pub force: bool,

    /// Show detailed output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct ContentSyncArgs {
    /// Force sync direction: disk -> database
    #[arg(long, conflicts_with = "force_to_disk")]
    pub force_to_db: bool,

    /// Force sync direction: database -> disk
    #[arg(long, conflicts_with = "force_to_db")]
    pub force_to_disk: bool,

    /// Override database URL
    #[arg(long)]
    pub database_url: Option<String>,

    /// Filter by source name
    #[arg(long)]
    pub source: Option<String>,

    /// Preview changes without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Delete orphaned records
    #[arg(long)]
    pub delete_orphans: bool,
}

#[derive(Args)]
pub struct SkillsSyncArgs {
    /// Force sync direction: disk -> database
    #[arg(long, conflicts_with = "force_to_disk")]
    pub force_to_db: bool,

    /// Force sync direction: database -> disk
    #[arg(long, conflicts_with = "force_to_db")]
    pub force_to_disk: bool,

    /// Override database URL
    #[arg(long)]
    pub database_url: Option<String>,

    /// Filter by skill name
    #[arg(long)]
    pub skill: Option<String>,

    /// Preview changes without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Delete orphaned records
    #[arg(long)]
    pub delete_orphans: bool,
}

pub async fn execute(cmd: SyncCommands) -> Result<()> {
    match cmd {
        SyncCommands::Push(args) => execute_cloud_sync(SyncDirection::Push, args).await,
        SyncCommands::Pull(args) => execute_cloud_sync(SyncDirection::Pull, args).await,
        SyncCommands::Local(cmd) => execute_local_sync(cmd).await,
    }
}

async fn execute_local_sync(cmd: LocalSyncCommands) -> Result<()> {
    match cmd {
        LocalSyncCommands::Content(args) => content::execute(args).await,
        LocalSyncCommands::Skills(args) => skills::execute(args).await,
    }
}

async fn execute_cloud_sync(direction: SyncDirection, args: SyncArgs) -> Result<()> {
    let creds = CredentialsBootstrap::require()
        .context("Cloud sync requires credentials. Run 'systemprompt cloud auth login'")?;

    let profile =
        ProfileBootstrap::get().context("Profile required for sync. Set SYSTEMPROMPT_PROFILE")?;

    if let Some(cloud) = &profile.cloud {
        if !cloud.enabled {
            bail!("Cloud features are disabled in this profile. Set cloud.enabled: true");
        }
    }

    let tenant_id = profile
        .cloud
        .as_ref()
        .and_then(|c| c.tenant_id.as_ref())
        .ok_or_else(|| anyhow!("No tenant configured. Run 'systemprompt cloud profile create'"))?;

    let services_path = profile.paths.services.clone();

    let database_url = SecretsBootstrap::get().ok().map(|s| s.database_url.clone());

    let config = SyncConfig {
        direction: direction.clone(),
        dry_run: args.dry_run,
        verbose: args.verbose,
        tenant_id: tenant_id.clone(),
        api_url: creds.api_url.clone(),
        api_token: creds.api_token.clone(),
        services_path,
        database_url,
    };

    print_header(&direction, args.dry_run);

    let service = SyncService::new(config);

    let spinner_msg = match direction {
        SyncDirection::Push => "Pushing to cloud...",
        SyncDirection::Pull => "Pulling from cloud...",
    };
    let spinner = CliService::spinner(spinner_msg);

    let results = service.sync_all().await?;

    spinner.finish_and_clear();
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
