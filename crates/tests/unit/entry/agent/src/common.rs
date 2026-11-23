//! Shared helpers for the `admin agents` and `core contexts` command tests: clap harnesses, member hosts holding the grants each
//! group's reserved path lists, and command contexts whose kernel holds
//! exactly the values the agent members provide to the commands.

use std::collections::HashMap;
use std::sync::Arc;

use clap::Parser;
use systemprompt_agent::services::agent_orchestration::status_view::AgentStatusView;
use systemprompt_agent::services::registry::AgentCardSite;
use systemprompt_agent_entry::cli::agents::AgentsCommands;
use systemprompt_agent_entry::cli::contexts::ContextsCommands;
use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{
    CliConfig, CommandDescriptor, CommandError, CommandHost, CommandOutput, KernelGrant,
    OutputFormat,
};
use systemprompt_loader::ServicesState;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{TestBootstrap, ensure_test_bootstrap, fixture_admitted};
use systemprompt_traits::{
    BackgroundTasks, DatabaseHandle, DynContextDirectory, DynSessionContextIssuer, SecretPresence,
};

#[derive(Debug, Parser)]
struct AgentsHarness {
    #[command(subcommand)]
    command: AgentsCommands,
}

#[derive(Debug, Parser)]
struct ContextsHarness {
    #[command(subcommand)]
    command: ContextsCommands,
}

pub fn parse_agents(args: &[&str]) -> AgentsCommands {
    AgentsHarness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .command
}

pub fn parse_contexts(args: &[&str]) -> ContextsCommands {
    ContextsHarness::try_parse_from(std::iter::once("contexts").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .command
}

pub fn json_cli() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

pub fn agents_host(ctx: &CommandContext) -> MemberHost<'_> {
    MemberHost::new(
        ctx,
        CommandDescriptor::FULL
            .with_grant(KernelGrant::ServicesTree)
            .with_grant(KernelGrant::CallerSession),
    )
}

pub fn contexts_host(ctx: &CommandContext) -> MemberHost<'_> {
    MemberHost::new(
        ctx,
        CommandDescriptor::FULL
            .with_grant(KernelGrant::CallerSession)
            .with_grant(KernelGrant::SessionContext),
    )
}

fn no_secrets() -> Arc<dyn SecretPresence> {
    Arc::new(HashMap::<String, String>::new())
}

pub fn boot_kernel() -> Kernel {
    let bootstrap = ensure_test_bootstrap();
    let services = ServicesState::load(&bootstrap.boot).expect("the seeded services tree loads");
    Kernel::detached(
        BackgroundTasks::new(),
        Dependencies::default()
            .with(services.view())
            .with(Arc::new(bootstrap.app_paths.clone()))
            .with(no_secrets()),
    )
}

pub fn agent_kernel(app: &AppContext) -> Kernel {
    let handle: Arc<dyn DatabaseHandle> =
        Arc::<systemprompt_database::Database>::clone(app.db_pool());
    let admitted = app.kernel().resolved();
    let status = admitted
        .get::<Arc<AgentStatusView>>()
        .expect("the agent member provides the status view");
    let site = admitted
        .get::<Arc<AgentCardSite>>()
        .expect("the agent member provides the card site");
    let directory = admitted
        .get::<DynContextDirectory>()
        .expect("the agent member provides the context directory");
    let issuer = admitted
        .get::<DynSessionContextIssuer>()
        .expect("the agent member provides the session context issuer");
    Kernel::new(
        handle,
        BackgroundTasks::new(),
        Dependencies::default()
            .with(app.services_state().view())
            .with(Arc::clone(app.app_paths_arc()))
            .with(status)
            .with(no_secrets())
            .with(site)
            .with(directory)
            .with(issuer),
    )
}

pub fn with_app(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> CommandContext {
    let boot = app.boot().clone();
    boot.install_services_root(boot.require_services_root());
    let admitted = Arc::clone(app.admitted());
    let kernel = agent_kernel(&app);
    CommandContext::with_app_context(cli, env, app)
        .with_boot(Some(boot))
        .with_admitted(Some(admitted))
        .with_kernel(Arc::new(kernel))
}

pub fn with_boot(cli: CliConfig) -> CommandContext {
    let boot = ensure_test_bootstrap().boot.clone();
    boot.install_services_root(boot.require_services_root());
    CommandContext::new(cli, EnvOverrides::default())
        .with_boot(Some(boot))
        .with_admitted(Some(fixture_admitted()))
        .with_kernel(Arc::new(boot_kernel()))
}

pub async fn app_on_database(bootstrap: &TestBootstrap, database_url: &str) -> Arc<AppContext> {
    let mut config = systemprompt_manifest::Config::clone(bootstrap.boot_config());
    config.database_url = database_url.to_owned();
    systemprompt_test_fixtures::fixture_app_context_on_bootstrap(
        bootstrap,
        config,
        fixture_admitted(),
    )
    .await
    .expect("fixture app context on the bootstrap's own paths")
}

pub async fn app_reloaded_on_database(
    bootstrap: &TestBootstrap,
    database_url: &str,
) -> Arc<AppContext> {
    systemprompt_test_fixtures::fixture_app_context_reloaded_on(
        bootstrap,
        database_url,
        fixture_admitted(),
    )
    .await
    .expect("fixture app context over the services tree as the test seeded it")
}

pub fn rendered(
    host: &dyn CommandHost,
    outcome: anyhow::Result<CommandOutput>,
) -> anyhow::Result<CommandOutput> {
    match &outcome {
        Ok(output) => host.render(output),
        Err(error) => {
            if let Some(output) = error
                .downcast_ref::<CommandError>()
                .and_then(CommandError::output)
            {
                host.render(output);
            }
        },
    }
    outcome
}
