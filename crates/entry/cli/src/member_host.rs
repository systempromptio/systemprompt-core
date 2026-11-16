//! The host a member's registered command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_extension::Resolved;
use systemprompt_extension::cli::{
    BoxFuture, ChildSupervisor, CliConfig, CommandDescriptor, CommandError, CommandHost,
    CommandOutput, DescribedCommand, KernelGrant, Prompter,
};
use systemprompt_manifest::{Config, Profile};
use systemprompt_runtime::Kernel;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ConfigApplyService, ConfigReconcileService};
use systemprompt_traits::{DatabaseHandle, Dependencies};
use tokio::sync::OnceCell;

use crate::context::CommandContext;
use crate::host::{boot_profile_config, boot_profile_refused};

/// The host a member's registered command runs against: the command context
/// with the grants the registry conferred on that command. It serves
/// [`CommandHost::boot_config`] only when the command holds
/// [`KernelGrant::BootProfile`]. Its [`CommandHost::resolved`] is the one
/// member-command resolver: it reads the command kernel's state but refuses
/// every sealed seed as missing. A command holding
/// [`KernelGrant::ConfigReconcile`] also resolves the kernel-built
/// `Arc<ConfigApplyService>`; no member resolves the application context.
pub struct MemberHost<'a> {
    ctx: &'a CommandContext,
    descriptor: CommandDescriptor,
    granted: OnceCell<Dependencies>,
}

impl std::fmt::Debug for MemberHost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberHost")
            .field("descriptor", &self.descriptor)
            .finish_non_exhaustive()
    }
}

impl<'a> MemberHost<'a> {
    #[must_use]
    pub const fn new(ctx: &'a CommandContext, descriptor: CommandDescriptor) -> Self {
        Self {
            ctx,
            descriptor,
            granted: OnceCell::const_new(),
        }
    }

    async fn config_reconcile_state(&self, kernel: &Kernel) -> Result<Dependencies, CommandError> {
        let app = self.ctx.app_context().await.map_err(CommandError::new)?;
        let reconcile = ConfigReconcileService::from_app_context(
            app,
            ConfigReconcileRepository::new(app.db_pool()),
        )
        .map_err(CommandError::new)?;
        Ok(kernel
            .state()
            .clone()
            .with(Arc::new(ConfigApplyService::new(reconcile))))
    }
}

impl CommandHost for MemberHost<'_> {
    fn output(&self) -> &CliConfig {
        CommandHost::output(self.ctx)
    }

    fn prompter(&self) -> &dyn Prompter {
        CommandHost::prompter(self.ctx)
    }

    fn is_database_scoped(&self) -> bool {
        CommandHost::is_database_scoped(self.ctx)
    }

    fn profile(&self) -> Result<&Profile, CommandError> {
        CommandHost::profile(self.ctx)
    }

    fn profile_path(&self) -> Result<&Path, CommandError> {
        CommandHost::profile_path(self.ctx)
    }

    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>> {
        CommandHost::db(self.ctx)
    }

    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>> {
        Box::pin(async move {
            let kernel = self.ctx.command_kernel().await.map_err(CommandError::new)?;
            let state = if self.descriptor.holds(KernelGrant::ConfigReconcile) {
                self.granted
                    .get_or_try_init(|| self.config_reconcile_state(kernel))
                    .await?
            } else {
                kernel.state()
            };
            Ok(Resolved::__member_command(&**kernel, state))
        })
    }

    fn children(&self) -> &dyn ChildSupervisor {
        CommandHost::children(self.ctx)
    }

    fn render(&self, output: &CommandOutput) {
        CommandHost::render(self.ctx, output);
    }

    fn describe_command_line(&self, command: &[String]) -> Result<DescribedCommand, CommandError> {
        CommandHost::describe_command_line(self.ctx, command)
    }

    fn env_profile(&self) -> Option<&str> {
        CommandHost::env_profile(self.ctx)
    }

    fn boot_config<'b>(
        &'b self,
        profile: &'b Path,
    ) -> BoxFuture<'b, Result<Arc<Config>, CommandError>> {
        if self.descriptor.holds(KernelGrant::BootProfile) {
            Box::pin(boot_profile_config(self.ctx, profile))
        } else {
            Box::pin(async move { Err(boot_profile_refused()) })
        }
    }
}
