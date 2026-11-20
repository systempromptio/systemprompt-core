//! The host a member's registered command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_extension::Resolved;
use systemprompt_extension::cli::{
    AdmittedViews, BoxFuture, CallerSession, ChildSupervisor, CliConfig, CommandDescriptor,
    CommandError, CommandHost, CommandOutput, DescribedCommand, KernelGrant, Prompter,
    ServicesTree,
};
use systemprompt_config::paths::StoragePaths;
use systemprompt_loader::load_boot::load_boot_from_path;
use systemprompt_manifest::{Config, Profile};
use systemprompt_runtime::Kernel;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ConfigApplyService, ConfigReconcileService};
use systemprompt_traits::{DatabaseHandle, Dependencies};
use tokio::sync::OnceCell;

use crate::context::CommandContext;
use crate::kernel::{KernelHost, TargetAdmissionError, admit_target};

/// The host a member's registered command runs against.
///
/// It is the command context with the grants the registry conferred on that
/// command. It serves
/// [`CommandHost::boot_config`] and [`CommandHost::profile`] only when the
/// command holds [`KernelGrant::BootProfile`],
/// [`CommandHost::services_tree`] only under [`KernelGrant::ServicesTree`]
/// and [`CommandHost::caller_session`] only under
/// [`KernelGrant::CallerSession`], and [`CommandHost::export_dir`] only under
/// [`KernelGrant::LocalExport`]. Its [`CommandHost::resolved`] is the one
/// member-command resolver: it reads the command kernel's state but refuses
/// every sealed seed as missing. A command holding
/// [`KernelGrant::ConfigReconcile`] run where an application context is
/// available also resolves the kernel-built `Arc<ConfigApplyService>`; no
/// member resolves the application context. It serves
/// [`CommandHost::admitted_views`] only when the command holds
/// [`KernelGrant::TargetAdmission`].
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

    fn serves_app_context(&self) -> bool {
        self.ctx.runtime.get().is_some()
            || (!self.ctx.is_database_scoped()
                && self.ctx.boot().is_ok()
                && self.ctx.admitted.is_some())
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
        if self.descriptor.holds(KernelGrant::BootProfile) {
            CommandHost::profile(self.ctx)
        } else {
            Err(grant_refused(KernelGrant::BootProfile))
        }
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
            let state = if self.descriptor.holds(KernelGrant::ConfigReconcile)
                && self.serves_app_context()
            {
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
            Box::pin(async move { Err(grant_refused(KernelGrant::BootProfile)) })
        }
    }

    fn admitted_views(&self, profile: &Path) -> Result<AdmittedViews, CommandError> {
        if !self.descriptor.holds(KernelGrant::TargetAdmission) {
            return Err(CommandError::new(TargetAdmissionError::Refused));
        }
        admit_target(self.ctx, profile).map_err(CommandError::new)
    }

    fn export_dir(&self) -> Result<PathBuf, CommandError> {
        if !self.descriptor.holds(KernelGrant::LocalExport) {
            return Err(grant_refused(KernelGrant::LocalExport));
        }
        let profile = self.ctx.profile().map_err(CommandError::new)?;
        let storage = StoragePaths::from_profile(&profile.paths).map_err(CommandError::new)?;
        Ok(storage.exports().to_path_buf())
    }

    fn services_tree(&self) -> Result<ServicesTree, CommandError> {
        if !self.descriptor.holds(KernelGrant::ServicesTree) {
            return Err(grant_refused(KernelGrant::ServicesTree));
        }
        granted_services_tree(self.ctx)
    }

    fn caller_session(&self) -> BoxFuture<'_, Result<CallerSession, CommandError>> {
        if !self.descriptor.holds(KernelGrant::CallerSession) {
            return Box::pin(async move { Err(grant_refused(KernelGrant::CallerSession)) });
        }
        Box::pin(async move {
            let session = crate::session::get_or_create_session(self.ctx)
                .await
                .map_err(CommandError::new)?;
            Ok(CallerSession::new(session.session_token().clone()))
        })
    }
}

fn granted_services_tree(ctx: &CommandContext) -> Result<ServicesTree, CommandError> {
    let root = KernelHost::services_root(ctx)?.path;
    let profile = ctx.profile().map_err(CommandError::new)?;
    Ok(ServicesTree::new(
        root,
        profile.server.api_external_url.clone(),
        profile.site.github_link.clone(),
    ))
}

pub(crate) async fn boot_profile_config(
    ctx: &CommandContext,
    profile: &Path,
) -> Result<Arc<Config>, CommandError> {
    let boot = load_boot_from_path(profile, ctx.secrets_sources().clone())
        .await
        .map_err(CommandError::new)?;
    let config = boot.try_init_config(None).map_err(CommandError::new)?;
    Ok(Arc::clone(config))
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("this command requires the `{0}` kernel grant")]
pub(crate) struct GrantRefused(pub(crate) KernelGrant);

pub(crate) fn grant_refused(grant: KernelGrant) -> CommandError {
    CommandError::new(GrantRefused(grant))
}
