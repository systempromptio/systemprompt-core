//! `admin users merge` command merging anonymous rows into an identified user.
//!
//! The merge runs the owner reassignments the admitted composition's members
//! contribute, each admitted over its owner's own tables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_extension::AdmittedOwnerReassignments;
use systemprompt_runtime::Kernel;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle, Dependencies};
use systemprompt_users::{UserAdminService, UserRepository, UserService};

use super::types::UserMergeOutput;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct MergeArgs {
    #[arg(long, help = "Source user (will be deleted after merge)")]
    pub source: String,

    #[arg(long, help = "Target user (will receive source's data)")]
    pub target: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

fn admitted_reassignments(
    host: &dyn KernelHost,
    pool: &DbPool,
) -> Result<AdmittedOwnerReassignments> {
    let handle: Arc<dyn DatabaseHandle> = Arc::<systemprompt_database::Database>::clone(pool);
    let kernel = Kernel::new(
        handle,
        BackgroundTasks::new(),
        Dependencies::new().with(Arc::clone(pool)),
    );
    Ok(AdmittedOwnerReassignments::of(
        host.admitted()?.built(),
        &kernel.resolved(),
    )?)
}

pub(super) async fn execute(args: MergeArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    if !args.yes {
        return Err(anyhow!(
            "This will merge the source user into the target user and DELETE the source. Use \
             --yes to confirm."
        ));
    }

    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)))
        .with_owner_reassignments(Arc::new(admitted_reassignments(host, &pool)?));
    let admin_service = UserAdminService::new(user_service.clone());

    let source_user = admin_service
        .find_user(&args.source)
        .await?
        .ok_or_else(|| anyhow!("Source user not found: {}", args.source))?;

    let target_user = admin_service
        .find_user(&args.target)
        .await?
        .ok_or_else(|| anyhow!("Target user not found: {}", args.target))?;

    if source_user.id == target_user.id {
        return Err(anyhow!("Source and target users cannot be the same"));
    }

    let result = user_service
        .merge_users(&source_user.id, &target_user.id)
        .await?;

    let output = UserMergeOutput {
        source_id: source_user.id.clone(),
        target_id: target_user.id.clone(),
        sessions_transferred: result.sessions,
        tasks_transferred: result.tasks,
        rows_transferred: result.total_rows,
        message: format!(
            "Merged user '{}' into '{}': {} sessions, {} tasks, {} rows transferred in total",
            source_user.name, target_user.name, result.sessions, result.tasks, result.total_rows
        ),
    };

    Ok(CommandOutput::card_value("Users Merged", &output))
}
