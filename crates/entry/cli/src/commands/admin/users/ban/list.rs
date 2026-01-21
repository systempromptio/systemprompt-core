use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_logging::CliService;
use systemprompt_users::BannedIpRepository;
use systemprompt_runtime::AppContext;
use tabled::{Table, Tabled};

use crate::commands::admin::users::types::{BanListOutput, BanSummary};

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "50")]
    pub limit: i64,

    #[arg(long)]
    pub source: Option<String>,
}

#[derive(Tabled)]
struct BanRow {
    #[tabled(rename = "IP Address")]
    ip: String,
    #[tabled(rename = "Reason")]
    reason: String,
    #[tabled(rename = "Permanent")]
    permanent: String,
    #[tabled(rename = "Count")]
    count: i32,
    #[tabled(rename = "Banned At")]
    banned_at: String,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(args: ListArgs, pool: &DbPool, config: &CliConfig) -> Result<()> {
    let ban_repository = BannedIpRepository::new(pool)?;

    let bans = match args.source {
        Some(ref source) => {
            ban_repository
                .list_bans_by_source(source, args.limit)
                .await?
        },
        None => ban_repository.list_active_bans(args.limit).await?,
    };

    let summaries: Vec<BanSummary> = bans
        .into_iter()
        .map(|b| BanSummary {
            ip_address: b.ip_address,
            reason: b.reason,
            banned_at: b.banned_at,
            expires_at: b.expires_at,
            is_permanent: b.is_permanent,
            ban_count: b.ban_count,
            ban_source: b.ban_source,
        })
        .collect();

    let output = BanListOutput {
        total: summaries.len(),
        bans: summaries,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("Active IP Bans");

        if output.bans.is_empty() {
            CliService::info("No active bans found");
        } else {
            let rows: Vec<BanRow> = output
                .bans
                .iter()
                .map(|b| BanRow {
                    ip: b.ip_address.clone(),
                    reason: truncate_string(&b.reason, 30),
                    permanent: if b.is_permanent { "yes" } else { "no" }.to_string(),
                    count: b.ban_count,
                    banned_at: b.banned_at.format("%Y-%m-%d %H:%M").to_string(),
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);

            CliService::info(&format!("Total: {} ban(s)", output.total));
        }
    }

    Ok(())
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
