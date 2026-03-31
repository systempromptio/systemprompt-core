mod backup;
mod restore;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::Command;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::commands::infrastructure::db;

#[derive(Debug, Subcommand)]
pub enum CloudDbCommands {
    #[command(about = "Run migrations on cloud database")]
    Migrate {
        #[arg(long, help = "Profile name")]
        profile: String,
    },

    #[command(about = "Execute SQL query (read-only) on cloud database")]
    Query {
        #[arg(long, help = "Profile name")]
        profile: String,
        sql: String,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
        #[arg(long)]
        format: Option<String>,
    },

    #[command(about = "Execute write operation on cloud database")]
    Execute {
        #[arg(long, help = "Profile name")]
        profile: String,
        sql: String,
        #[arg(long)]
        format: Option<String>,
    },

    #[command(about = "Validate cloud database schema")]
    Validate {
        #[arg(long, help = "Profile name")]
        profile: String,
    },

    #[command(about = "Show cloud database connection status")]
    Status {
        #[arg(long, help = "Profile name")]
        profile: String,
    },

    #[command(about = "Show cloud database info")]
    Info {
        #[arg(long, help = "Profile name")]
        profile: String,
    },

    #[command(about = "List all tables in cloud database")]
    Tables {
        #[arg(long, help = "Profile name")]
        profile: String,
        #[arg(long, help = "Filter tables by pattern")]
        filter: Option<String>,
    },

    #[command(about = "Describe table schema in cloud database")]
    Describe {
        #[arg(long, help = "Profile name")]
        profile: String,
        table_name: String,
    },

    #[command(about = "Get row count for a table in cloud database")]
    Count {
        #[arg(long, help = "Profile name")]
        profile: String,
        table_name: String,
    },

    #[command(about = "List all indexes in cloud database")]
    Indexes {
        #[arg(long, help = "Profile name")]
        profile: String,
        #[arg(long, help = "Filter by table name")]
        table: Option<String>,
    },

    #[command(about = "Show cloud database and table sizes")]
    Size {
        #[arg(long, help = "Profile name")]
        profile: String,
    },

    #[command(about = "Backup cloud database using pg_dump")]
    Backup {
        #[arg(long, help = "Profile name")]
        profile: String,

        #[arg(
            long,
            default_value = "custom",
            help = "Backup format: custom, sql, directory"
        )]
        format: BackupFormat,

        #[arg(
            long,
            help = "Output file path (default: backups/<profile>-<timestamp>.<ext>)"
        )]
        output: Option<String>,
    },

    #[command(about = "Restore cloud database from a backup file")]
    Restore {
        #[arg(long, help = "Profile name")]
        profile: String,

        #[arg(help = "Path to backup file")]
        file: String,

        #[arg(short = 'y', long, help = "Skip confirmation prompt")]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BackupFormat {
    #[value(help = "pg_dump custom format (-Fc), supports parallel restore")]
    Custom,
    #[value(help = "Plain SQL text format (-Fp), human-readable")]
    Sql,
    #[value(help = "Directory format (-Fd), supports parallel dump and restore")]
    Directory,
}

impl CloudDbCommands {
    fn profile_name(&self) -> &str {
        match self {
            Self::Migrate { profile }
            | Self::Query { profile, .. }
            | Self::Execute { profile, .. }
            | Self::Validate { profile }
            | Self::Status { profile }
            | Self::Info { profile }
            | Self::Tables { profile, .. }
            | Self::Describe { profile, .. }
            | Self::Count { profile, .. }
            | Self::Indexes { profile, .. }
            | Self::Size { profile }
            | Self::Backup { profile, .. }
            | Self::Restore { profile, .. } => profile,
        }
    }

    fn into_db_command(self) -> Option<db::DbCommands> {
        match self {
            Self::Migrate { .. } => Some(db::DbCommands::Migrate),
            Self::Query {
                sql,
                limit,
                offset,
                format,
                ..
            } => Some(db::DbCommands::Query {
                sql,
                limit,
                offset,
                format,
            }),
            Self::Execute { sql, format, .. } => Some(db::DbCommands::Execute { sql, format }),
            Self::Validate { .. } => Some(db::DbCommands::Validate),
            Self::Status { .. } => Some(db::DbCommands::Status),
            Self::Info { .. } => Some(db::DbCommands::Info),
            Self::Tables { filter, .. } => Some(db::DbCommands::Tables { filter }),
            Self::Describe { table_name, .. } => Some(db::DbCommands::Describe { table_name }),
            Self::Count { table_name, .. } => Some(db::DbCommands::Count { table_name }),
            Self::Indexes { table, .. } => Some(db::DbCommands::Indexes { table }),
            Self::Size { .. } => Some(db::DbCommands::Size),
            Self::Backup { .. } | Self::Restore { .. } => None,
        }
    }
}

pub async fn execute(cmd: CloudDbCommands, config: &CliConfig) -> Result<()> {
    let profile_name = cmd.profile_name().to_string();
    let db_url = load_cloud_database_url(&profile_name)?;
    execute_inner(cmd, &profile_name, &db_url, config).await
}

pub async fn execute_with_database_url(
    cmd: CloudDbCommands,
    database_url: &str,
    config: &CliConfig,
) -> Result<()> {
    let profile_name = cmd.profile_name().to_string();
    execute_inner(cmd, &profile_name, database_url, config).await
}

async fn execute_inner(
    cmd: CloudDbCommands,
    profile_name: &str,
    db_url: &str,
    config: &CliConfig,
) -> Result<()> {
    match &cmd {
        CloudDbCommands::Backup { format, output, .. } => {
            return backup::execute(profile_name, db_url, *format, output.as_deref());
        },
        CloudDbCommands::Restore { file, yes, .. } => {
            return restore::execute(profile_name, db_url, file, *yes, config);
        },
        _ => {},
    }

    let db_ctx = DatabaseContext::from_url(db_url).await?;
    let db_cmd = cmd
        .into_db_command()
        .ok_or_else(|| anyhow!("Unexpected command variant"))?;

    db::execute_with_db(db_cmd, &db_ctx, config).await
}

fn load_cloud_database_url(profile_name: &str) -> Result<String> {
    let ctx = ProjectContext::discover();
    let profile_dir = ctx.profile_dir(profile_name);

    if !profile_dir.exists() {
        return Err(anyhow!("Profile '{}' not found", profile_name));
    }

    let secrets_path = ProfilePath::Secrets.resolve(&profile_dir);
    let secrets = systemprompt_models::Secrets::load_from_path(&secrets_path)
        .with_context(|| format!("Failed to load secrets for profile '{}'", profile_name))?;

    Ok(secrets.effective_database_url(true).to_string())
}

fn ensure_pg_tool(tool: &str) -> Result<()> {
    match Command::new(tool).arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => bail!(
            "'{}' not found. Install PostgreSQL client tools:\n  apt install postgresql-client",
            tool
        ),
    }
}

fn find_pg_dump() -> Result<PathBuf> {
    for version in [17, 16, 15, 14] {
        let path = PathBuf::from(format!("/usr/lib/postgresql/{}/bin/pg_dump", version));
        if path.exists() {
            return Ok(path);
        }
    }
    ensure_pg_tool("pg_dump")?;
    Ok(PathBuf::from("pg_dump"))
}

fn find_pg_restore() -> Result<PathBuf> {
    for version in [17, 16, 15, 14] {
        let path = PathBuf::from(format!("/usr/lib/postgresql/{}/bin/pg_restore", version));
        if path.exists() {
            return Ok(path);
        }
    }
    ensure_pg_tool("pg_restore")?;
    Ok(PathBuf::from("pg_restore"))
}

fn adjust_ssl_mode(database_url: &str) -> String {
    database_url.replace("sslmode=require", "sslmode=prefer")
}
