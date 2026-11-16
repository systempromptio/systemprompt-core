//! CLI runtime entry point and bootstrap helpers.
//!
//! Owns argument parsing (`args`), profile/secrets bootstrap (`bootstrap`),
//! and cloud routing ([`routing`], [`profile_routing`]). [`run`] is the
//! process entry point: it assembles the command tree from the kernel groups
//! and the [`CommandRegistry`], parses, resolves the selected command's
//! [`CommandDescriptor`], bootstraps what that descriptor asks for, and
//! dispatches.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod args;
mod bootstrap;
mod composed;
pub mod confirmation;
mod db_url;
mod direct;
mod dispatch;
pub mod interrupt;
pub mod profile_routing;
pub mod routing;
pub mod routing_decision;
mod structured_output;
mod tls;

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_config::SecretsSources;
use systemprompt_loader::subprocess::{self, ApiServerStamp, ForegroundChildren};
use systemprompt_logging::OutputState;
use systemprompt_models::BuildIdentity;

use crate::context::CommandContext;
use crate::env_overrides::EnvOverrides;
use crate::kernel::{CommandRegistry, ResolveError};
use composed::admit_for;
pub use composed::{AdmitFn, CliRunError, admit_unprofiled, refuse_disabled, run_composed};
use direct::{DirectDatabase, direct_database_url, run_with_database_url};
use dispatch::{Invocation, Selected, dispatch_command};
use systemprompt_extension::cli::{CliConfig, CommandDescriptor, OutputFormat};

pub(super) async fn run_with(
    build: BuildIdentity,
    registry: CommandRegistry,
    secrets: SecretsSources,
    admit: AdmitFn,
) -> Result<()> {
    tls::install_crypto_provider();
    let registry = Arc::new(registry);
    let output = OutputState::new();
    let root = args::command(&registry)?;
    let matches = root.clone().get_matches();
    let invocation = match Invocation::parse(&registry, root, &matches) {
        Ok(invocation) => invocation,
        Err(ResolveError::Parse(error)) => error.exit(),
        Err(error @ ResolveError::Registry(_)) => return Err(error.into()),
    };
    let confirm = confirmation::ConfirmationRequest::from_matches(
        &matches,
        &args::reconstruct_args(),
        invocation.cli.profile_opts.profile.clone(),
    );
    let env = EnvOverrides::from_process_env();
    let confirm = confirm.routed_by_gateway(env.is_remote_cli);
    let shield = interrupt::InterruptShield::default();
    let children = ForegroundChildren::default();
    let guards = RunGuards {
        shield: shield.clone(),
        children: children.clone(),
        build,
        secrets,
        admit,
        registry: Arc::clone(&registry),
    };
    let outcome = if invocation.descriptor().owns_signals() {
        Box::pin(run_inner(&output, invocation, confirm, guards, env)).await
    } else {
        let command = Box::pin(run_inner(&output, invocation, confirm, guards, env));
        interrupt::run_interruptible(shield, children, command).await
    };
    structured_output::finalize(&output, &outcome);
    outcome
}

async fn run_inner(
    output: &OutputState,
    invocation: Invocation,
    confirm: confirmation::ConfirmationRequest,
    guards: RunGuards,
    env: EnvOverrides,
) -> Result<()> {
    let desc = invocation.descriptor();
    stamp_api_server_identity(&desc)?;

    let cli_config = configure_output(output, &invocation, &env);

    let database_url_override = invocation.cli.database.database_url.clone();
    if let Some(database_url) = direct_database_url(&invocation)? {
        let direct = confirmation::ConfirmationRequest {
            target: Some("the --database-url database".to_owned()),
            ..confirm.clone()
        };
        confirmation::gate(&desc, &direct, &cli_config)?;
        systemprompt_logging::init_deferred_console_logging(output.clone());
        return Box::pin(run_with_database_url(
            invocation.into_selected(),
            cli_config,
            env,
            DirectDatabase {
                url: &database_url,
                credentials: None,
            },
            guards,
        ))
        .await;
    }

    if desc.database() {
        systemprompt_logging::init_deferred_console_logging(output.clone());
    } else {
        let effective_level = resolve_log_level(&cli_config, &env, !desc.child_host());
        systemprompt_logging::init_console_logging_with_level(effective_level.as_deref(), output);
    }

    let mut credentials = None;
    let mut services = None;
    let mut profile = None;
    let mut boot = None;
    if desc.profile() {
        let booted = profile_routing::bootstrap_profile(
            &invocation.cli,
            &desc,
            &cli_config,
            &env,
            &confirm,
            &guards.secrets,
        )
        .await?;
        let loaded = booted.credentials;
        services = booted.services;
        profile = booted.profile;
        boot = booted.boot;
        match booted.outcome {
            profile_routing::BootstrapOutcome::RemoteExecuted => return Ok(()),
            profile_routing::BootstrapOutcome::ExternalDbUrl(external_db_url) => {
                return Box::pin(run_with_database_url(
                    invocation.into_selected(),
                    cli_config,
                    env,
                    DirectDatabase {
                        url: &external_db_url,
                        credentials: loaded,
                    },
                    guards,
                ))
                .await;
            },
            profile_routing::BootstrapOutcome::ContinueLocal => credentials = loaded,
        }
    }

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
    } = guards;
    let admitted = admit_for(&invocation, profile.as_deref(), admit)?;
    if desc.paths() && !desc.skip_validation()
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
        .with_registry(registry)
        .with_cloud_credentials(credentials)
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

fn configure_output(
    output: &OutputState,
    invocation: &Invocation,
    env: &EnvOverrides,
) -> CliConfig {
    output.set_startup_mode(invocation.is_bare());
    let cli = &invocation.cli;
    let cli_config = args::build_cli_config(cli, env);
    output.set_structured_output(cli_config.output_format() != OutputFormat::Table);
    if cli.display.no_color || !cli_config.should_use_color() {
        console::set_colors_enabled(false);
    }
    cli_config
}

fn resolve_log_level(
    cli_config: &CliConfig,
    env: &EnvOverrides,
    discover_profile: bool,
) -> Option<String> {
    if env.rust_log.is_some() {
        return None;
    }

    if let Some(level) = cli_config.verbosity.as_tracing_filter() {
        return Some(level.to_owned());
    }

    if !discover_profile {
        return Some("warn".to_owned());
    }

    if let Ok(resolved) = bootstrap::resolve_profile(cli_config.profile_override.as_deref(), env)
        && let Some(log_level) = bootstrap::try_load_log_level(&resolved.path)
    {
        return Some(log_level.as_tracing_filter().to_owned());
    }

    Some("warn".to_owned())
}

struct RunGuards {
    shield: interrupt::InterruptShield,
    children: ForegroundChildren,
    build: BuildIdentity,
    secrets: SecretsSources,
    admit: AdmitFn,
    registry: Arc<CommandRegistry>,
}

impl RunGuards {
    fn apply(self, ctx: CommandContext, selected: &Selected) -> Result<CommandContext> {
        let admitted = admit_unprofiled(selected.registered(), self.admit)?;
        Ok(ctx
            .with_admitted(Some(Arc::new(admitted)))
            .with_interrupt_shield(self.shield)
            .with_foreground_children(self.children)
            .with_build_identity(self.build)
            .with_secrets_sources(self.secrets)
            .with_registry(self.registry))
    }
}

fn stamp_api_server_identity(desc: &CommandDescriptor) -> Result<()> {
    if !desc.serves_api() {
        return Ok(());
    }
    match subprocess::stamp_api_server()
        .context("Failed to re-execute the API server with its identity marker")?
    {
        ApiServerStamp::Stamped | ApiServerStamp::Unverifiable => Ok(()),
    }
}
