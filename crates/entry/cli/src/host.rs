//! The command context as the host a contributed command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use systemprompt_extension::cli::{
    BoxFuture, ChildSupervisor, CliConfig, CommandDescriptor, CommandError, CommandHost,
    CommandOutput, DescribedCommand, KernelGrant, Prompter,
};
use systemprompt_extension::{Admitted, AdmittedSchedule, Resolved};
use systemprompt_traits::{AppContext as _, BackgroundTasks, Dependencies};
use systemprompt_config::SecretsSources;
use systemprompt_loader::BootContext;
use systemprompt_loader::load_boot::load_boot_from_path;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile, Secrets};
use systemprompt_runtime::{AppContext, AppContextBuilder, Kernel};
use systemprompt_traits::DatabaseHandle;

use crate::context::CommandContext;
use crate::env_overrides::EnvOverrides;
use crate::kernel::{CommandRegistry, classify_command_line};

/// What the CLI kernel's own commands may ask of it beyond [`CommandHost`].
///
/// The kernel's groups (`core`, `infra`, `web`, `plugins`) read boot-only
/// data a member's command never reaches: the active services root, the
/// admitted composition, the loaded secrets, the profile's configuration and
/// the sanctioned environment a child process is started with, and
/// `infra services serve` the server's context builder over the same boot and
/// admitted composition, the application context, the boot context, the
/// runner's environment overrides and secrets sources, and a profile booted
/// at a path. None of these is placed in the state a member's command
/// resolves. Each value is
/// read from the boot context the command kernel's state is seeded from, so
/// there is one source. Only the kernel host implements it; it is not part of
/// the extension contract.
pub trait KernelHost: CommandHost {
    fn services_root(&self) -> Result<ActiveServicesRoot, CommandError>;
    fn admitted(&self) -> Result<&Admitted, CommandError>;
    fn secrets(&self) -> Result<Arc<Secrets>, CommandError>;
    fn config(&self) -> Result<Arc<Config>, CommandError>;
    fn child_env(&self) -> Result<Vec<(&'static str, String)>, CommandError>;
    fn server_builder(&self) -> Result<AppContextBuilder, CommandError>;
    fn app_context(&self) -> BoxFuture<'_, Result<Arc<AppContext>, CommandError>>;
    fn boot(&self) -> Result<Arc<BootContext>, CommandError>;
    fn env_overrides(&self) -> &EnvOverrides;
    fn secrets_sources(&self) -> &SecretsSources;
    fn boot_config<'a>(
        &'a self,
        profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>>;
}

impl CommandContext {
    #[must_use]
    pub fn with_admitted(mut self, admitted: Option<Arc<Admitted>>) -> Self {
        self.admitted = admitted;
        self
    }

    #[must_use]
    pub fn with_registry(mut self, registry: Arc<CommandRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    #[must_use]
    pub fn with_kernel(mut self, kernel: Arc<Kernel>) -> Self {
        self.kernel = tokio::sync::OnceCell::new_with(Some(kernel));
        self
    }

    #[must_use]
    pub fn with_detached_kernel(self) -> Self {
        let mut seed = self.command_seed(Dependencies::new());
        if let Some(admitted) = &self.admitted {
            seed.insert(AdmittedSchedule::of(admitted.built()));
        }
        let kernel = Kernel::detached(BackgroundTasks::new(), seed);
        self.with_kernel(Arc::new(kernel))
    }

    fn command_seed(&self, base: Dependencies) -> Dependencies {
        let mut seed = base.with(self.interrupt_shield().clone());
        if let Ok(profile) = self.loaded_profile() {
            seed.insert(Arc::clone(profile));
        }
        if let Some(services) = self.services() {
            seed.insert(services.clone());
        }
        seed
    }

    async fn command_kernel(&self) -> anyhow::Result<&Arc<Kernel>> {
        self.kernel
            .get_or_try_init(|| async {
                let app = self.app_context().await?;
                let state = self.command_seed(app.kernel().state().clone());
                Ok(Arc::new(Kernel::new(
                    app.database_handle(),
                    app.background_tasks().clone(),
                    state,
                )))
            })
            .await
    }

    pub fn admitted(&self) -> anyhow::Result<&Arc<Admitted>> {
        self.admitted
            .as_ref()
            .context("This command requires the admitted composition")
    }
}

impl CommandHost for CommandContext {
    fn output(&self) -> &CliConfig {
        &self.cli
    }

    fn prompter(&self) -> &dyn Prompter {
        CommandContext::prompter(self)
    }

    fn is_database_scoped(&self) -> bool {
        CommandContext::is_database_scoped(self)
    }

    fn profile(&self) -> Result<&Profile, CommandError> {
        CommandContext::profile(self).map_err(CommandError::new)
    }

    fn profile_path(&self) -> Result<&Path, CommandError> {
        self.loaded_profile()
            .map(|loaded| loaded.path())
            .map_err(CommandError::new)
    }

    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>> {
        Box::pin(async move {
            let pool = self.db_pool().await.map_err(CommandError::new)?;
            let handle: Arc<dyn DatabaseHandle> = pool;
            Ok(handle)
        })
    }

    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>> {
        Box::pin(async move {
            let kernel = self.command_kernel().await.map_err(CommandError::new)?;
            Ok(kernel.resolved())
        })
    }

    fn children(&self) -> &dyn ChildSupervisor {
        &self.foreground_children
    }

    fn render(&self, output: &CommandOutput) {
        crate::shared::render_result(output, &self.cli);
    }

    fn describe_command_line(&self, command: &[String]) -> Result<DescribedCommand, CommandError> {
        let registry = self.registry.as_ref().ok_or_else(|| {
            CommandError::new(anyhow::anyhow!(
                "Describing a command line requires the command registry"
            ))
        })?;
        let profile = CommandContext::profile(self).map_err(CommandError::new)?;
        classify_command_line(registry, profile, command).map_err(CommandError::new)
    }

    fn env_profile(&self) -> Option<&str> {
        self.env.profile.as_deref()
    }

    fn boot_config<'a>(
        &'a self,
        _profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>> {
        Box::pin(async move { Err(boot_profile_refused()) })
    }
}

impl KernelHost for CommandContext {
    fn services_root(&self) -> Result<ActiveServicesRoot, CommandError> {
        self.boot()
            .map_err(CommandError::new)?
            .services_root()
            .cloned()
            .ok_or_else(|| {
                CommandError::new(anyhow::anyhow!(
                    "This command requires the active services root, which the profile boot did \
                     not resolve"
                ))
            })
    }

    fn admitted(&self) -> Result<&Admitted, CommandError> {
        CommandContext::admitted(self)
            .map(|admitted| &**admitted)
            .map_err(CommandError::new)
    }

    fn secrets(&self) -> Result<Arc<Secrets>, CommandError> {
        CommandContext::secrets(self)
            .map(|service| service.get())
            .map_err(CommandError::new)
    }

    fn config(&self) -> Result<Arc<Config>, CommandError> {
        CommandContext::config(self)
            .map(Arc::clone)
            .map_err(CommandError::new)
    }

    fn child_env(&self) -> Result<Vec<(&'static str, String)>, CommandError> {
        let profile = self
            .loaded_profile()
            .map_err(CommandError::new)?
            .path()
            .to_string_lossy()
            .into_owned();
        let pepper = CommandContext::secrets(self)
            .map_err(CommandError::new)?
            .oauth_at_rest_pepper();
        Ok(vec![
            ("SYSTEMPROMPT_PROFILE", profile),
            ("OAUTH_AT_REST_PEPPER", pepper),
        ])
    }

    fn server_builder(&self) -> Result<AppContextBuilder, CommandError> {
        self.app_context_builder().map_err(CommandError::new)
    }

    fn app_context(&self) -> BoxFuture<'_, Result<Arc<AppContext>, CommandError>> {
        Box::pin(async move {
            CommandContext::app_context(self)
                .await
                .map(Arc::clone)
                .map_err(CommandError::new)
        })
    }

    fn boot(&self) -> Result<Arc<BootContext>, CommandError> {
        CommandContext::boot(self)
            .map(|boot| Arc::new(boot.clone()))
            .map_err(CommandError::new)
    }

    fn env_overrides(&self) -> &EnvOverrides {
        &self.env
    }

    fn secrets_sources(&self) -> &SecretsSources {
        CommandContext::secrets_sources(self)
    }

    fn boot_config<'a>(
        &'a self,
        profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>> {
        Box::pin(boot_profile_config(self, profile))
    }
}

async fn boot_profile_config(
    ctx: &CommandContext,
    profile: &Path,
) -> Result<Arc<Config>, CommandError> {
    let boot = load_boot_from_path(profile, ctx.secrets_sources().clone())
        .await
        .map_err(CommandError::new)?;
    let config = boot.try_init_config(None).map_err(CommandError::new)?;
    Ok(Arc::clone(config))
}

fn boot_profile_refused() -> CommandError {
    CommandError::new(anyhow::anyhow!(
        "Booting a profile requires the `{}` kernel grant",
        KernelGrant::BootProfile
    ))
}

/// The host a member's registered command runs against: the command context
/// with the grants the registry conferred on that command. It serves
/// [`CommandHost::boot_config`] only when the command holds
/// [`KernelGrant::BootProfile`].
pub struct MemberHost<'a> {
    ctx: &'a CommandContext,
    descriptor: CommandDescriptor,
}

impl<'a> MemberHost<'a> {
    #[must_use]
    pub const fn new(ctx: &'a CommandContext, descriptor: CommandDescriptor) -> Self {
        Self { ctx, descriptor }
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
        CommandHost::resolved(self.ctx)
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
