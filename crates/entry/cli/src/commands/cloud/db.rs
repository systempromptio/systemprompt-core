use anyhow::{anyhow, bail, Context, Result};
use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::Command;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;
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
            return execute_backup(profile_name, db_url, *format, output.as_deref());
        },
        CloudDbCommands::Restore { file, yes, .. } => {
            return execute_restore(profile_name, db_url, file, *yes, config);
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
    if !secrets_path.exists() {
        return Err(anyhow!(
            "No secrets.json found for profile '{}'",
            profile_name
        ));
    }

    let secrets_content = std::fs::read_to_string(&secrets_path)
        .with_context(|| format!("Failed to read {}", secrets_path.display()))?;

    let secrets: serde_json::Value =
        serde_json::from_str(&secrets_content).with_context(|| "Failed to parse secrets.json")?;

    secrets["external_database_url"]
        .as_str()
        .or_else(|| secrets["database_url"].as_str())
        .map(String::from)
        .ok_or_else(|| {
            anyhow!(
                "No database_url or external_database_url in secrets.json for profile '{}'",
                profile_name
            )
        })
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

const fn backup_extension(format: BackupFormat) -> &'static str {
    match format {
        BackupFormat::Custom => "dump",
        BackupFormat::Sql => "sql",
        BackupFormat::Directory => "dir",
    }
}

const fn pg_dump_format_flag(format: BackupFormat) -> &'static str {
    match format {
        BackupFormat::Custom => "-Fc",
        BackupFormat::Sql => "-Fp",
        BackupFormat::Directory => "-Fd",
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

fn adjust_ssl_mode(database_url: &str) -> String {
    database_url.replace("sslmode=require", "sslmode=prefer")
}

fn execute_backup(
    profile_name: &str,
    database_url: &str,
    format: BackupFormat,
    output: Option<&str>,
) -> Result<()> {
    let pg_dump = find_pg_dump()?;

    let output_path = if let Some(p) = output {
        PathBuf::from(p)
    } else {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
        let ext = backup_extension(format);
        let dir = PathBuf::from("backups");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create {}", dir.display()))?;
        dir.join(format!("{}-{}.{}", profile_name, timestamp, ext))
    };

    CliService::section("Cloud Database Backup");
    CliService::key_value("Profile", profile_name);
    CliService::key_value("Format", &format!("{:?}", format));
    CliService::key_value("Output", &output_path.display().to_string());

    let adjusted_url = adjust_ssl_mode(database_url);
    let spinner = CliService::spinner("Running pg_dump...");

    let result = Command::new(&pg_dump)
        .arg(pg_dump_format_flag(format))
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg("-f")
        .arg(&output_path)
        .arg(&adjusted_url)
        .output()
        .context("Failed to execute pg_dump")?;

    spinner.finish_and_clear();

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("pg_dump failed:\n{}", stderr);
    }

    let size = if output_path.is_dir() {
        dir_size(&output_path)
    } else {
        std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0)
    };

    CliService::success(&format!(
        "Backup complete: {} ({})",
        output_path.display(),
        format_size(size)
    ));

    Ok(())
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

fn execute_restore(
    profile_name: &str,
    database_url: &str,
    file: &str,
    skip_confirm: bool,
    config: &CliConfig,
) -> Result<()> {
    let file_path = PathBuf::from(file);
    if !file_path.exists() {
        bail!("Backup file not found: {}", file);
    }

    let is_custom_or_dir = std::path::Path::new(file)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dump"))
        || file_path.is_dir();

    if is_custom_or_dir {
        find_pg_restore()?;
    } else {
        ensure_pg_tool("psql")?;
    }

    CliService::section("Cloud Database Restore");
    CliService::key_value("Profile", profile_name);
    CliService::key_value("File", file);
    CliService::warning("This will overwrite data in the cloud database!");

    if !skip_confirm {
        if !config.is_interactive() {
            bail!("Restore requires -y flag in non-interactive mode");
        }

        let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(format!(
                "Restore backup to cloud database for profile '{}'?",
                profile_name
            ))
            .default(false)
            .interact()?;

        if !confirm {
            CliService::info("Cancelled");
            return Ok(());
        }
    }

    let adjusted_url = adjust_ssl_mode(database_url);
    let spinner = CliService::spinner("Restoring database...");

    let result = if is_custom_or_dir {
        let pg_restore = find_pg_restore()?;
        Command::new(pg_restore)
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("--clean")
            .arg("--if-exists")
            .arg("-d")
            .arg(&adjusted_url)
            .arg(file)
            .output()
            .context("Failed to execute pg_restore")?
    } else {
        Command::new("psql")
            .arg(&adjusted_url)
            .arg("-f")
            .arg(file)
            .output()
            .context("Failed to execute psql")?
    };

    spinner.finish_and_clear();

    if result.status.success() {
        CliService::success("Database restored successfully");
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        if stderr.contains("ERROR") {
            bail!("Restore failed:\n{}", stderr);
        }
        CliService::warning("Restore completed with warnings:");
        CliService::info(&stderr.chars().take(500).collect::<String>());
    }

    Ok(())
}

fn dir_size(path: &PathBuf) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
