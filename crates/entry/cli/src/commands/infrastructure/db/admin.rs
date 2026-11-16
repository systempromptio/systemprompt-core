//! `infra db` admin subcommand wiring.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::DatabaseAdminService;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::AppContext;
use systemprompt_users::{PromoteResult, UserAdminService, UserService};

use crate::shared::CommandOutput;

use super::text_format::format_bytes;
use super::types::{DbAssignAdminOutput, DbStatusOutput};

pub(super) use super::admin_migrate::{
    MigrateOptions, execute_migrate, execute_migrate_standalone,
};
pub(super) use super::admin_migrate_down::{execute_migrate_down, execute_migrate_down_standalone};
pub(super) use super::admin_migrate_mark_applied::{
    MarkAppliedArgs, execute_migrate_mark_applied, execute_migrate_mark_applied_standalone,
};
pub(super) use super::admin_migrate_plan::{execute_migrate_plan, execute_migrate_plan_standalone};
pub(super) use super::admin_migrate_repair::{
    RepairArgs, execute_migrate_repair, execute_migrate_repair_standalone,
};
pub(super) use super::admin_migrate_status::{
    execute_migrate_status, execute_migrate_status_standalone,
};
pub(super) use super::admin_migrations::{execute_migrations, execute_migrations_standalone};

pub(super) async fn execute_assign_admin(ctx: &AppContext, user: &str) -> Result<CommandOutput> {
    let user_service = UserService::new(Arc::clone(ctx.user_repository()));
    let user_admin = UserAdminService::new(user_service);

    let (output, note) = match user_admin.promote_to_admin(user).await? {
        PromoteResult::Promoted(u, new_roles) => {
            let message = format!("Admin role assigned to user '{}' ({})", u.name, u.email);
            let output = DbAssignAdminOutput {
                user_id: u.id,
                name: u.name,
                email: u.email,
                roles: new_roles,
                already_admin: false,
                message: message.clone(),
            };
            (output, NoticeLine::new("success", message))
        },
        PromoteResult::AlreadyAdmin(u) => {
            let message = format!("User '{}' already has admin role", u.name);
            let output = DbAssignAdminOutput {
                user_id: u.id,
                name: u.name,
                email: u.email,
                roles: u.roles,
                already_admin: true,
                message: message.clone(),
            };
            (output, NoticeLine::new("warning", message))
        },
        PromoteResult::UserNotFound => {
            return Err(anyhow!("User '{}' not found", user));
        },
    };

    Ok(CommandOutput::card_value("Database Admin", &output).with_note(note))
}

pub(super) async fn execute_status(admin: &DatabaseAdminService) -> Result<CommandOutput> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to connect to database")?;

    let output = DbStatusOutput {
        status: "connected".to_owned(),
        version: info.version.clone(),
        tables: info.tables.len(),
        size: format_bytes(info.size as i64),
    };

    Ok(CommandOutput::card_value("Database Status", &output)
        .with_note(NoticeLine::new("success", "Database connection: OK")))
}
