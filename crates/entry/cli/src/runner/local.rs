//! The local half of a run: admission, validation and dispatch over the
//! booted profile, and the guards every run carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_config::SecretsSources;
use systemprompt_extension::cli::CliConfig;
use systemprompt_loader::subprocess::ForegroundChildren;
use systemprompt_models::BuildIdentity;

use super::composed::{Admission, AdmitFn, admit_direct, admit_for};
use super::dispatch::{Invocation, Selected, dispatch_command};
use super::{bootstrap, confirmation, interrupt};
use crate::context::CommandContext;
use crate::env_overrides::EnvOverrides;
use crate::kernel::CommandRegistry;

pub(super) struct LocalBoot {
    pub(super) confirm: confirmation::ConfirmationRequest,
    pub(super) services: Option<systemprompt_loader::ServicesState>,
    pub(super) profile: Option<Arc<systemprompt_config::LoadedProfile>>,
    pub(super) boot: Option<systemprompt_loader::BootContext>,
    pub(super) database_url_override: Option<String>,
}

pub(super) async fn run_local(
    invocation: Invocation,
    cli_config: CliConfig,
    env: EnvOverrides,
    guards: RunGuards,
    local: LocalBoot,
) -> Result<()> {
    let desc = invocation.descriptor();
    let LocalBoot {
        confirm,
        services,
        profile,
        boot,
        database_url_override,
    } = local;
    if !desc.profile() {
        confirmation::gate(&desc, &confirm, &cli_config)?;
    }

    let RunGuards {
        shield,
        children,
        build,
        secrets,
        admit,
        registry,
        cloud,
    } = guards;
    let (admitted, deferred_admit) = match admit_for(&invocation, profile.as_deref(), admit)? {
        Admission::Admitted(admitted) => (Some(admitted), None),
        Admission::Deferred(admit) => (None, admit),
    };
    if desc.paths()
        && !desc.skip_validation()
        && let (Some(boot), Some(admitted)) = (&boot, &admitted)
    {
        bootstrap::run_validation(boot, admitted.built())?;
    }
    let ctx = CommandContext::new(cli_config, env)
        .with_interrupt_shield(shield)
        .with_foreground_children(children)
        .with_build_identity(build)
        .with_secrets_sources(secrets)
        .with_admitted(admitted)
        .with_admit(deferred_admit)
        .with_registry(registry)
        .with_cloud_access(cloud)
        .with_database_url_override(database_url_override)
        .with_services(services)
        .with_profile(profile)
        .with_boot(boot);
    let ctx = if desc.database() {
        ctx
    } else {
        ctx.with_detached_kernel()
    };
    Box::pin(dispatch_command(invocation.into_selected(), &ctx)).await
}

pub(super) struct RunGuards {
    pub(super) shield: interrupt::InterruptShield,
    pub(super) children: ForegroundChildren,
    pub(super) build: BuildIdentity,
    pub(super) secrets: SecretsSources,
    pub(super) admit: AdmitFn,
    pub(super) registry: Arc<CommandRegistry>,
    pub(super) cloud: Option<systemprompt_extension::cli::DynCloudAccess>,
}

impl RunGuards {
    pub(super) fn apply(
        self,
        ctx: CommandContext,
        selected: &Selected,
        profile: Option<&systemprompt_manifest::Profile>,
    ) -> Result<CommandContext> {
        let admitted = admit_direct(selected.registered(), profile, self.admit)?;
        Ok(ctx
            .with_admitted(Some(Arc::new(admitted)))
            .with_interrupt_shield(self.shield)
            .with_foreground_children(self.children)
            .with_build_identity(self.build)
            .with_secrets_sources(self.secrets)
            .with_registry(self.registry)
            .with_cloud_access(self.cloud))
    }
}
