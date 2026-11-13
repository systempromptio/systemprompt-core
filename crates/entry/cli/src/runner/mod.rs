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
pub mod confirmation;
mod db_url;
pub mod interrupt;
pub mod profile_routing;
pub mod routing;
pub mod routing_decision;
mod structured_output;

use anyhow::{Context, Result, bail};
use clap::{ArgMatches, Command, FromArgMatches};
use systemprompt_cloud::CloudCredentials;
use systemprompt_loader::subprocess::{self, ApiServerStamp, ForegroundChildren};
use systemprompt_logging::OutputState;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::{CliConfig, OutputFormat};
use crate::commands::{admin, analytics, cloud, core, infrastructure, plugins, web};
use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DbUrlRouting, DescribeCommand};
use crate::env_overrides::EnvOverrides;
use crate::kernel::{CommandRegistry, RegisteredCommand};

pub async fn run() -> Result<()> {
    install_tls_crypto_provider();
    let output = OutputState::new();
    let registry = CommandRegistry::collected()?;
    let root = args::command(&registry)?;
    let matches = root.clone().get_matches();
    let invocation = Invocation::parse(&registry, root, &matches).map_err(|e| e.exit())?;
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
    };
    let outcome = if invocation.descriptor().serves_api() {
        Box::pin(run_inner(&output, invocation, confirm, guards, env)).await
    } else {
        let command = Box::pin(run_inner(&output, invocation, confirm, guards, env));
        interrupt::run_interruptible(shield, children, command).await
    };
    structured_output::finalize(&output, &outcome);
    outcome
}

// Why: reqwest 0.13 is built with `rustls-no-provider` so the tree carries ring
// only (sqlx already uses it); rustls then needs a process-wide provider before
// the first client is built, or reqwest panics.
fn install_tls_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return;
    }
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("rustls crypto provider was installed concurrently");
    }
}

/// The parsed command line: global options plus the selected command, which
/// is either a kernel command (`cli.command`), a registered group, or none.
struct Invocation {
    cli: args::Cli,
    registered: Option<RegisteredCommand>,
    root: Command,
}

impl Invocation {
    fn parse(
        registry: &CommandRegistry,
        root: Command,
        matches: &ArgMatches,
    ) -> Result<Self, clap::Error> {
        if let Some(registered) = registry.resolve(matches)? {
            return Ok(Self {
                cli: args::Cli::from_globals(matches)?,
                registered: Some(registered),
                root,
            });
        }
        Ok(Self {
            cli: args::Cli::from_arg_matches(matches)?,
            registered: None,
            root,
        })
    }

    fn descriptor(&self) -> CommandDescriptor {
        match (&self.registered, &self.cli.command) {
            (Some(registered), _) => registered.descriptor(),
            (None, Some(command)) => command.descriptor(),
            (None, None) => CommandDescriptor::FULL,
        }
    }

    const fn is_bare(&self) -> bool {
        self.registered.is_none() && self.cli.command.is_none()
    }

    const fn db_url_routing(&self) -> Option<DbUrlRouting> {
        match (&self.registered, &self.cli.command) {
            (Some(registered), _) => Some(registered.db_url_routing()),
            (None, Some(command)) => Some(command.db_url_routing()),
            (None, None) => None,
        }
    }

    fn into_selected(self) -> Selected {
        match (self.registered, self.cli.command) {
            (Some(registered), _) => Selected::Registered(registered),
            (None, Some(command)) => Selected::Kernel(Box::new(command)),
            (None, None) => Selected::Bare(Box::new(self.root)),
        }
    }
}

enum Selected {
    Kernel(Box<args::Commands>),
    Registered(RegisteredCommand),
    Bare(Box<Command>),
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
        let effective_level = resolve_log_level(&cli_config, &env);
        systemprompt_logging::init_console_logging_with_level(effective_level.as_deref(), output);
    }

    let mut credentials = None;
    let mut services = None;
    let mut profile = None;
    let mut boot = None;
    if desc.profile() {
        let booted =
            profile_routing::bootstrap_profile(&invocation.cli, &desc, &cli_config, &env, &confirm)
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

    let ctx = CommandContext::new(cli_config, env)
        .with_interrupt_shield(guards.shield)
        .with_foreground_children(guards.children)
        .with_cloud_credentials(credentials)
        .with_database_url_override(database_url_override)
        .with_services(services)
        .with_profile(profile)
        .with_boot(boot);
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

fn direct_database_url(invocation: &Invocation) -> Result<Option<String>> {
    let Some(database_url) = invocation.cli.database.database_url.clone() else {
        return Ok(None);
    };
    match invocation.db_url_routing() {
        Some(DbUrlRouting::Direct) => Ok(Some(database_url)),
        Some(DbUrlRouting::Unsupported) => bail!(
            "This command cannot run with --database-url; it requires full profile \
             initialization. Remove --database-url."
        ),
        Some(DbUrlRouting::ProfileDriven) | None => Ok(None),
    }
}

fn resolve_log_level(cli_config: &CliConfig, env: &EnvOverrides) -> Option<String> {
    if env.rust_log.is_some() {
        return None;
    }

    if let Some(level) = cli_config.verbosity.as_tracing_filter() {
        return Some(level.to_owned());
    }

    if let Ok(resolved) = bootstrap::resolve_profile(cli_config.profile_override.as_deref(), env)
        && let Some(log_level) = bootstrap::try_load_log_level(&resolved.path)
    {
        return Some(log_level.as_tracing_filter().to_owned());
    }

    Some("warn".to_owned())
}

async fn dispatch_command(selected: Selected, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() {
        let accepted = match &selected {
            Selected::Kernel(command) => matches!(
                **command,
                args::Commands::Core(_)
                    | args::Commands::Infra(_)
                    | args::Commands::Admin(_)
                    | args::Commands::Analytics(_)
            ),
            Selected::Registered(registered) => registered.db_url_routing() == DbUrlRouting::Direct,
            Selected::Bare(_) => {
                bail!("No subcommand provided. Use --help to see available commands.")
            },
        };
        if !accepted {
            bail!("This command requires full profile initialization. Remove --database-url flag.");
        }
    }

    match selected {
        Selected::Kernel(command) => match *command {
            args::Commands::Core(cmd) => Box::pin(core::execute(cmd, ctx)).await?,
            args::Commands::Infra(cmd) => infrastructure::execute(cmd, ctx).await?,
            args::Commands::Admin(cmd) => Box::pin(admin::execute(cmd, ctx)).await?,
            args::Commands::Cloud(cmd) => cloud::execute(cmd, ctx).await?,
            args::Commands::Analytics(cmd) => analytics::execute(cmd, ctx).await?,
            args::Commands::Web(cmd) => web::execute(cmd, ctx)?,
            args::Commands::Plugins(cmd) => {
                Box::pin(plugins::execute(cmd, ctx)).await?;
            },
        },
        Selected::Registered(registered) => registered.run(ctx).await?,
        Selected::Bare(root) => {
            (*root).get_matches_from(["systemprompt", "--help"]);
        },
    }

    Ok(())
}

struct RunGuards {
    shield: interrupt::InterruptShield,
    children: ForegroundChildren,
}

struct DirectDatabase<'a> {
    url: &'a str,
    credentials: Option<CloudCredentials>,
}

async fn run_with_database_url(
    selected: Selected,
    cli_config: CliConfig,
    env: EnvOverrides,
    database: DirectDatabase<'_>,
    guards: RunGuards,
) -> Result<()> {
    let DirectDatabase {
        url: database_url,
        credentials,
    } = database;
    let db_ctx = DatabaseContext::from_url(database_url)
        .await
        .context("Failed to connect to database")?;

    systemprompt_logging::init_logging(&db_ctx.db_pool_arc(), None);

    let ctx = CommandContext::with_database(cli_config, env, db_ctx, database_url.to_owned())
        .with_cloud_credentials(credentials)
        .with_interrupt_shield(guards.shield)
        .with_foreground_children(guards.children);
    Box::pin(dispatch_command(selected, &ctx)).await
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
