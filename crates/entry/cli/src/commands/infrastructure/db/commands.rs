use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum DbCommands {
    #[command(about = "Execute SQL query (read-only)")]
    Query {
        sql: String,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
        #[arg(long)]
        format: Option<String>,
    },
    #[command(about = "Execute write operation (INSERT, UPDATE, DELETE)")]
    Execute {
        sql: String,
        #[arg(long)]
        format: Option<String>,
    },
    #[command(about = "List all tables with row counts and sizes")]
    Tables {
        #[arg(long, help = "Filter tables by pattern")]
        filter: Option<String>,
    },
    #[command(about = "Describe table schema with columns and indexes")]
    Describe { table_name: String },
    #[command(about = "Show database information")]
    Info,
    #[command(about = "Run database migrations")]
    Migrate {
        #[arg(
            long,
            help = "Continue past migration checksum mismatches with a warning instead of \
                    erroring (use with caution)"
        )]
        allow_checksum_drift: bool,
    },
    #[command(about = "Revert the most recently applied migrations for an extension")]
    MigrateDown {
        #[arg(help = "Extension ID")]
        extension: String,
        #[arg(help = "Number of migrations to revert")]
        count: u32,
    },
    #[command(
        about = "Squash an extension's migrations 1..=N into a baseline at version 0 (dry-run by \
                 default)"
    )]
    MigrateSquash {
        #[arg(long, help = "Extension ID whose migrations should be squashed")]
        extension: String,
        #[arg(
            long,
            help = "Squash migrations with version 1..=through into the baseline"
        )]
        through: u32,
        #[arg(
            long,
            help = "Apply the squash (write baseline file + rewrite DB rows). Without this flag, \
                    the command is a dry-run."
        )]
        apply: bool,
    },
    #[command(about = "Show migration status and history")]
    Migrations {
        #[command(subcommand)]
        cmd: MigrationsCommands,
    },
    #[command(
        about = "Show pending migrations (dry-run / plan, no DB writes)",
        name = "migrate-plan"
    )]
    MigratePlan {
        #[arg(help = "Filter by extension ID (default: all extensions)")]
        extension: Option<String>,
        #[arg(long, help = "Emit JSON instead of a text table")]
        json: bool,
    },
    #[command(
        about = "Detailed introspectable migration status (applied, pending, drift)",
        name = "migrate-status"
    )]
    MigrateStatus {
        #[arg(help = "Filter by extension ID (default: all extensions)")]
        extension: Option<String>,
        #[arg(long, help = "Emit JSON instead of a text table")]
        json: bool,
    },
    #[command(about = "Assign admin role to a user")]
    AssignAdmin { user: String },
    #[command(about = "Show database connection status")]
    Status,
    #[command(about = "Validate database schema against expected tables")]
    Validate,
    #[command(about = "Get row count for a table")]
    Count { table_name: String },
    #[command(about = "List all indexes")]
    Indexes {
        #[arg(long, help = "Filter by table name")]
        table: Option<String>,
    },
    #[command(about = "Show database and table sizes")]
    Size,
    #[command(about = "Diff live schema against extension declarations")]
    Doctor,
}

#[derive(Debug, Subcommand)]
pub enum MigrationsCommands {
    #[command(about = "Show migration status for all extensions")]
    Status,
    #[command(about = "Show migration history for an extension")]
    History {
        #[arg(help = "Extension ID")]
        extension: String,
    },
}
