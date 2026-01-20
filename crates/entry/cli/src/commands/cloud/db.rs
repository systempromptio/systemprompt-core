use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
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
            | Self::Size { profile } => profile,
        }
    }

    fn into_db_command(self) -> db::DbCommands {
        match self {
            Self::Migrate { .. } => db::DbCommands::Migrate,
            Self::Query {
                sql,
                limit,
                offset,
                format,
                ..
            } => db::DbCommands::Query {
                sql,
                limit,
                offset,
                format,
            },
            Self::Execute { sql, format, .. } => db::DbCommands::Execute { sql, format },
            Self::Validate { .. } => db::DbCommands::Validate,
            Self::Status { .. } => db::DbCommands::Status,
            Self::Info { .. } => db::DbCommands::Info,
            Self::Tables { filter, .. } => db::DbCommands::Tables { filter },
            Self::Describe { table_name, .. } => db::DbCommands::Describe { table_name },
            Self::Count { table_name, .. } => db::DbCommands::Count { table_name },
            Self::Indexes { table, .. } => db::DbCommands::Indexes { table },
            Self::Size { .. } => db::DbCommands::Size,
        }
    }
}

pub async fn execute(cmd: CloudDbCommands, config: &CliConfig) -> Result<()> {
    let profile_name = cmd.profile_name().to_string();
    let db_url = load_cloud_database_url(&profile_name)?;
    let db_ctx = DatabaseContext::from_url(&db_url).await?;
    let db_cmd = cmd.into_db_command();

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

    secrets["database_url"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| {
            anyhow!(
                "No database_url in secrets.json for profile '{}'",
                profile_name
            )
        })
}
