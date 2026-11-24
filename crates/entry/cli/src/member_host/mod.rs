//! The host a member's registered command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, ExtensionId};

use systemprompt_config::paths::StoragePaths;
use systemprompt_extension::cli::{
    AdmittedViews, BoxFuture, CallerSession, ChildSupervisor, CliConfig, CommandDescriptor,
    CommandError, CommandHost, CommandOutput, DeploymentSigningKeys, DescribedCommand,
    GrantedPlane, IdentityCliSettings, KernelGrant, Prompter, ServicesTree,
};
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::{Resolved, StandingTag};
use systemprompt_loader::load_boot::load_boot_from_path;
use systemprompt_manifest::{Config, Profile};
use systemprompt_runtime::Kernel;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ConfigApplyService, ConfigReconcileService};
use systemprompt_traits::{DatabaseHandle, Dependencies};
use tokio::sync::OnceCell;

use crate::context::CommandContext;

pub(crate) use self::granted::IdentitySettingsRefused;
use self::granted::{granted_services_tree, identity_settings, writable_plane};

mod granted;
use crate::kernel::{KernelHost, KernelSigningKeys, TargetAdmissionError, admit_target};

/// The host a member's registered command runs against.
///
/// It is the command context with the grants the registry conferred on that
/// command. It serves
/// [`CommandHost::boot_config`] and [`CommandHost::profile`] only when the
/// command holds [`KernelGrant::BootProfile`],
/// [`CommandHost::services_tree`] only under [`KernelGrant::ServicesTree`]
/// and [`CommandHost::caller_session`] only under
/// [`KernelGrant::CallerSession`], [`CommandHost::use_session_context`]
/// only under [`KernelGrant::SessionContext`], and [`CommandHost::export_dir`]
/// only under [`KernelGrant::LocalExport`], [`CommandHost::signing_keys`]
/// only under [`KernelGrant::SigningKeys`] and
/// [`CommandHost::require_writable_plane`] only under
/// [`KernelGrant::ConfigPlaneWriter`] for that plane. Its
/// [`CommandHost::resolved`] is the one member-command resolver: it reads the
/// command kernel's state but refuses every sealed seed as missing. A command
/// holding [`KernelGrant::ConfigReconcile`] run where an application context is
/// available also resolves the kernel-built `Arc<ConfigApplyService>`; no
/// member resolves the application context. It serves
/// [`CommandHost::admitted_views`] only when the command holds
/// [`KernelGrant::TargetAdmission`]. Resolved for the command's owner
/// ([`MemberHost::owned_by`]), its resolver also serves a value recorded under
/// a conferral row whose core holder is that owner, and it serves
/// [`CommandHost::identity_settings`] only to a command `identity-http` owns
/// with core standing.
pub struct MemberHost<'a> {
    ctx: &'a CommandContext,
    descriptor: CommandDescriptor,
    owner: Option<(ExtensionId, StandingTag)>,
    granted: OnceCell<Dependencies>,
}

impl std::fmt::Debug for MemberHost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberHost")
            .field("descriptor", &self.descriptor)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl<'a> MemberHost<'a> {
    #[must_use]
    pub const fn new(ctx: &'a CommandContext, descriptor: CommandDescriptor) -> Self {
        Self {
            ctx,
            descriptor,
            owner: None,
            granted: OnceCell::const_new(),
        }
    }

    #[must_use]
    pub fn owned_by(mut self, owner: ExtensionId, standing: StandingTag) -> Self {
        self.owner = Some((owner, standing));
        self
    }

    fn owned_by_identity(&self) -> bool {
        self.owner.as_ref().is_some_and(|(owner, standing)| {
            *standing == StandingTag::Core && owner.as_str() == CoreId::IdentityHttp.id()
        })
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
            ConfigReconcileRepository::new(
                app.kernel()
                    .state()
                    .get::<systemprompt_database::DbPool>()
                    .map_err(CommandError::new)?,
            ),
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
            Ok(match &self.owner {
                Some((owner, standing)) => {
                    Resolved::__member_command_owned(&**kernel, state, owner, *standing)
                },
                None => Resolved::__member_command(&**kernel, state),
            })
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
            Ok(session.caller_session())
        })
    }

    fn use_session_context(&self, context: ContextId) -> BoxFuture<'_, Result<(), CommandError>> {
        if !self.descriptor.holds(KernelGrant::SessionContext) {
            return Box::pin(async move { Err(grant_refused(KernelGrant::SessionContext)) });
        }
        Box::pin(async move {
            Box::pin(crate::session::use_session_context(self.ctx, context))
                .await
                .map_err(CommandError::new)
        })
    }

    fn signing_keys(&self) -> Result<Arc<dyn DeploymentSigningKeys>, CommandError> {
        if !self.descriptor.holds(KernelGrant::SigningKeys) {
            return Err(grant_refused(KernelGrant::SigningKeys));
        }
        let boot = KernelHost::boot(self.ctx)?;
        let security = self
            .ctx
            .profile()
            .map_err(CommandError::new)?
            .security
            .clone();
        Ok(Arc::new(KernelSigningKeys::new(boot, security)))
    }

    fn require_writable_plane(
        &self,
        plane: GrantedPlane,
    ) -> BoxFuture<'_, Result<(), CommandError>> {
        if !self.descriptor.holds(KernelGrant::ConfigPlaneWriter(plane)) {
            return Box::pin(
                async move { Err(grant_refused(KernelGrant::ConfigPlaneWriter(plane))) },
            );
        }
        Box::pin(writable_plane(self.ctx, plane))
    }

    fn identity_settings(&self) -> Result<IdentityCliSettings, CommandError> {
        if !self.owned_by_identity() {
            return Err(CommandError::new(IdentitySettingsRefused));
        }
        identity_settings(self.ctx)
    }
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
