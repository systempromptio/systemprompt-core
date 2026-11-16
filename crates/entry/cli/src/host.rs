//! The command context as the host a contributed command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use systemprompt_extension::cli::{
    BoxFuture, ChildSupervisor, CliConfig, CommandError, CommandHost, CommandOutput,
    DescribedCommand, Prompter,
};
use systemprompt_extension::{Admitted, AdmittedSchedule, Resolved};
use systemprompt_traits::{AppContext as _, BackgroundTasks, Dependencies};
use systemprompt_loader::load_boot::load_boot_from_path;
use systemprompt_manifest::{Config, Profile};
use systemprompt_runtime::Kernel;
use systemprompt_traits::DatabaseHandle;

use crate::context::CommandContext;
use crate::kernel::{CommandRegistry, classify_command_line};

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
        let mut seed = base
            .with(self.secrets_sources().clone())
            .with(self.interrupt_shield().clone());
        if let Ok(profile) = self.loaded_profile() {
            seed.insert(Arc::clone(profile));
        }
        if let Ok(boot) = self.boot() {
            seed.insert(Arc::new(boot.clone()));
            if let Ok(config) = boot.config() {
                seed.insert(Arc::clone(config));
            }
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
                let state = self
                    .command_seed(app.kernel().state().clone())
                    .with(Arc::clone(app));
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
            .context("This command requires the composition admitted against a profile")
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
        profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>> {
        Box::pin(async move {
            let boot = load_boot_from_path(profile, self.secrets_sources().clone())
                .await
                .map_err(CommandError::new)?;
            let config = boot.try_init_config(None).map_err(CommandError::new)?;
            Ok(Arc::clone(config))
        })
    }
}
