//! The command context as the host a contributed command runs against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;
use systemprompt_identifiers::ContextId;

use anyhow::Context;
use systemprompt_config::SecretsSources;
use systemprompt_extension::cli::{
    AdmittedViews, BoxFuture, CallerSession, ChildSupervisor, CliConfig, CommandError, CommandHost,
    CommandOutput, DeploymentSigningKeys, DescribedCommand, DynCloudAccess, GrantedPlane,
    IdentityCliSettings, KernelGrant, Prompter, ServicesTree,
};
use systemprompt_extension::{Admitted, Resolved};
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile, Secrets};
use systemprompt_runtime::{AppContext, AppContextBuilder, Kernel};
use systemprompt_traits::DatabaseHandle;

use crate::context::CommandContext;
use crate::env_overrides::EnvOverrides;
use crate::kernel::{CommandRegistry, classify_command_line};
use crate::member_host::{IdentitySettingsRefused, boot_profile_config, grant_refused};

/// What the CLI kernel's own commands may ask of it beyond [`CommandHost`].
///
/// The kernel's groups (`core`, `infra`, `web`, `plugins`) read boot-only
/// data a member's command never reaches: the active services root, the
/// admitted composition, the loaded secrets, the profile's configuration and
/// the sanctioned environment a child process is started with, and
/// `infra services serve` the server's context builder over the same boot and
/// admitted composition, the application context, the boot context, the
/// runner's environment overrides and secrets sources, and a profile booted
/// at a path, and the cloud member's access when the composition has one.
/// None of these is placed in the state a member's command
/// resolves. Each value is read from the boot context the command kernel's
/// state is seeded from, so there is one source. Only the kernel host
/// implements it; it is not part of the extension contract.
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
    fn take_admit(&self) -> Result<systemprompt_runtime::AdmitFn, CommandError>;
    fn cloud_access(&self) -> Option<&DynCloudAccess>;
    fn linked(&self) -> Result<Admitted, CommandError>;
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
        Self::prompter(self)
    }

    fn is_database_scoped(&self) -> bool {
        Self::is_database_scoped(self)
    }

    fn profile(&self) -> Result<&Profile, CommandError> {
        Self::profile(self).map_err(CommandError::new)
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
        let profile = Self::profile(self).map_err(CommandError::new)?;
        classify_command_line(registry, profile, command).map_err(CommandError::new)
    }

    fn env_profile(&self) -> Option<&str> {
        self.env.profile.as_deref()
    }

    fn boot_config<'a>(
        &'a self,
        _profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>> {
        Box::pin(async move { Err(grant_refused(KernelGrant::BootProfile)) })
    }

    fn admitted_views(&self, _profile: &Path) -> Result<AdmittedViews, CommandError> {
        Err(CommandError::new(
            crate::kernel::TargetAdmissionError::Refused,
        ))
    }

    fn services_tree(&self) -> Result<ServicesTree, CommandError> {
        Err(grant_refused(KernelGrant::ServicesTree))
    }

    fn export_dir(&self) -> Result<std::path::PathBuf, CommandError> {
        Err(grant_refused(KernelGrant::LocalExport))
    }

    fn caller_session(&self) -> BoxFuture<'_, Result<CallerSession, CommandError>> {
        Box::pin(async move { Err(grant_refused(KernelGrant::CallerSession)) })
    }

    fn use_session_context(&self, _context: ContextId) -> BoxFuture<'_, Result<(), CommandError>> {
        Box::pin(async move { Err(grant_refused(KernelGrant::SessionContext)) })
    }

    fn signing_keys(&self) -> Result<Arc<dyn DeploymentSigningKeys>, CommandError> {
        Err(grant_refused(KernelGrant::SigningKeys))
    }

    fn require_writable_plane(
        &self,
        plane: GrantedPlane,
    ) -> BoxFuture<'_, Result<(), CommandError>> {
        Box::pin(async move { Err(grant_refused(KernelGrant::ConfigPlaneWriter(plane))) })
    }

    fn identity_settings(&self) -> Result<IdentityCliSettings, CommandError> {
        Err(CommandError::new(IdentitySettingsRefused))
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
        Self::admitted(self)
            .map(|admitted| &**admitted)
            .map_err(CommandError::new)
    }

    fn secrets(&self) -> Result<Arc<Secrets>, CommandError> {
        Self::secrets(self)
            .map(|service| service.get())
            .map_err(CommandError::new)
    }

    fn config(&self) -> Result<Arc<Config>, CommandError> {
        Self::config(self)
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
        let pepper = Self::secrets(self)
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
            Self::app_context(self)
                .await
                .map(Arc::clone)
                .map_err(CommandError::new)
        })
    }

    fn boot(&self) -> Result<Arc<BootContext>, CommandError> {
        Self::boot(self)
            .map(|boot| Arc::new(boot.clone()))
            .map_err(CommandError::new)
    }

    fn env_overrides(&self) -> &EnvOverrides {
        &self.env
    }

    fn secrets_sources(&self) -> &SecretsSources {
        Self::secrets_sources(self)
    }

    fn boot_config<'a>(
        &'a self,
        profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>> {
        Box::pin(boot_profile_config(self, profile))
    }

    fn take_admit(&self) -> Result<systemprompt_runtime::AdmitFn, CommandError> {
        let refused = || anyhow::anyhow!("Only a child-host command admits the composition");
        let admit = Self::take_admit(self).ok_or_else(|| CommandError::new(refused()))?;
        Ok(Box::new(move |profile: &Profile| {
            admit(Some(&profile.extensions))
        }))
    }

    fn cloud_access(&self) -> Option<&DynCloudAccess> {
        Self::cloud_access(self)
    }

    fn linked(&self) -> Result<Admitted, CommandError> {
        let refused =
            || anyhow::anyhow!("Only a command holding target admission admits the composition");
        let admit = Self::take_admit(self).ok_or_else(|| CommandError::new(refused()))?;
        crate::runner::admit_unprofiled(None, admit).map_err(CommandError::new)
    }
}
