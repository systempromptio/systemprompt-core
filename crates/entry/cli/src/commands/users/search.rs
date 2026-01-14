use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;
use tabled::{Table, Tabled};

use super::types::{UserListOutput, UserSummary};

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}

#[derive(Tabled)]
struct UserRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Email")]
    email: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Roles")]
    roles: String,
}

pub async fn execute(args: SearchArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    let users = user_service.search(&args.query, args.limit).await?;
    let total = users.len() as i64;

    let output = UserListOutput {
        users: users
            .iter()
            .map(|u| UserSummary {
                id: u.id.to_string(),
                name: u.name.clone(),
                email: u.email.clone(),
                status: u.status.clone(),
                roles: u.roles.clone(),
                created_at: u.created_at,
            })
            .collect(),
        total,
        limit: args.limit,
        offset: 0,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section(&format!("Search Results for '{}'", args.query));

        if users.is_empty() {
            CliService::info("No users found");
        } else {
            let rows: Vec<UserRow> = users
                .iter()
                .map(|u| UserRow {
                    id: u.id.to_string(),
                    name: u.name.clone(),
                    email: u.email.clone(),
                    status: u.status.clone().unwrap_or_else(|| "unknown".to_string()),
                    roles: u.roles.join(", "),
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);

            CliService::info(&format!("Found {} user(s)", total));
        }
    }

    Ok(())
}
