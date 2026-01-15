use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{UserAdminService, UserService};
use systemprompt_runtime::AppContext;

use super::types::{SessionSummary, UserActivityOutput, UserDetailOutput};

#[derive(Debug, Args)]
pub struct ShowArgs {
    pub identifier: String,

    #[arg(long)]
    pub sessions: bool,

    #[arg(long)]
    pub activity: bool,
}

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    let user = admin_service.find_user(&args.identifier).await?;

    let Some(user) = user else {
        CliService::error(&format!("User not found: {}", args.identifier));
        return Err(anyhow!("User not found"));
    };

    let sessions = if args.sessions {
        let user_sessions = user_service.list_sessions(&user.id).await?;
        Some(
            user_sessions
                .into_iter()
                .map(|s| SessionSummary {
                    session_id: s.session_id,
                    ip_address: s.ip_address,
                    user_agent: s.user_agent,
                    device_type: s.device_type,
                    started_at: s.started_at,
                    last_activity_at: s.last_activity_at,
                    is_active: s.ended_at.is_none(),
                })
                .collect(),
        )
    } else {
        None
    };

    let activity = if args.activity {
        let user_activity = user_service.get_activity(&user.id).await?;
        Some(UserActivityOutput {
            user_id: user_activity.user_id,
            last_active: user_activity.last_active,
            session_count: user_activity.session_count,
            task_count: user_activity.task_count,
            message_count: user_activity.message_count,
        })
    } else {
        None
    };

    let output = UserDetailOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        full_name: user.full_name.clone(),
        display_name: user.display_name.clone(),
        status: user.status.clone(),
        email_verified: user.email_verified,
        roles: user.roles.clone(),
        is_bot: user.is_bot,
        is_scanner: user.is_scanner,
        created_at: user.created_at,
        updated_at: user.updated_at,
        sessions,
        activity,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("User Details");
        CliService::key_value("ID", output.id.as_str());
        CliService::key_value("Name", &output.name);
        CliService::key_value("Email", &output.email);

        if let Some(ref full_name) = output.full_name {
            CliService::key_value("Full Name", full_name);
        }

        if let Some(ref display_name) = output.display_name {
            CliService::key_value("Display Name", display_name);
        }

        CliService::key_value("Status", output.status.as_deref().unwrap_or("unknown"));
        CliService::key_value("Roles", &output.roles.join(", "));
        CliService::key_value(
            "Email Verified",
            &output.email_verified.unwrap_or(false).to_string(),
        );
        CliService::key_value("Is Bot", &output.is_bot.to_string());
        CliService::key_value("Is Scanner", &output.is_scanner.to_string());

        if let Some(ref created_at) = output.created_at {
            CliService::key_value("Created", &created_at.to_rfc3339());
        }

        if let Some(ref updated_at) = output.updated_at {
            CliService::key_value("Updated", &updated_at.to_rfc3339());
        }

        if let Some(ref sessions) = output.sessions {
            CliService::section("Sessions");
            if sessions.is_empty() {
                CliService::info("No sessions found");
            } else {
                for session in sessions {
                    let status = if session.is_active { "active" } else { "ended" };
                    CliService::key_value(
                        session.session_id.as_str(),
                        &format!(
                            "{} | {} | {}",
                            status,
                            session.ip_address.as_deref().unwrap_or("unknown"),
                            session.device_type.as_deref().unwrap_or("unknown")
                        ),
                    );
                }
            }
        }

        if let Some(ref activity) = output.activity {
            CliService::section("Activity");
            CliService::key_value("Sessions", &activity.session_count.to_string());
            CliService::key_value("Tasks", &activity.task_count.to_string());
            CliService::key_value("Messages", &activity.message_count.to_string());
            if let Some(ref last_active) = activity.last_active {
                CliService::key_value("Last Active", &last_active.to_rfc3339());
            }
        }
    }

    Ok(())
}
